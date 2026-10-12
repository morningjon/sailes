//! Demo data: a small consultancy's book of business, with suggestions waiting for review.
//! Everything here is fictional.

use chrono::{Datelike, Duration, Utc, Weekday};
use serde_json::{json, Value};
use sqlx::{PgConnection, PgPool};
use uuid::Uuid;

use crate::routes::{
    activities::{self, NewActivity},
    proposals::{self, NewProposal},
};

struct Ctx {
    stages: Vec<(String, Uuid)>,
    pipeline: Uuid,
}

impl Ctx {
    fn stage(&self, name: &str) -> Uuid {
        self.stages
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, id)| *id)
            .expect("stage exists")
    }
}

async fn company(conn: &mut PgConnection, v: Value) -> anyhow::Result<Uuid> {
    let m = v.as_object().unwrap().clone();
    Ok(crate::fields::insert_object(conn, &crate::fields::COMPANY, &m).await?)
}

async fn contact(conn: &mut PgConnection, v: Value) -> anyhow::Result<Uuid> {
    let m = v.as_object().unwrap().clone();
    Ok(crate::fields::insert_object(conn, &crate::fields::CONTACT, &m).await?)
}

#[allow(clippy::too_many_arguments)]
async fn deal(
    conn: &mut PgConnection,
    ctx: &Ctx,
    name: &str,
    amount: i64,
    stage: &str,
    close_in_days: i64,
    company: Uuid,
    contacts: &[Uuid],
    next_step: Option<&str>,
) -> anyhow::Result<Uuid> {
    let close = (Utc::now() + Duration::days(close_in_days)).date_naive();
    let id: Uuid = sqlx::query_scalar(
        "insert into deals (name, amount, pipeline_id, stage_id, close_date, company_id, next_step, priority)
         values ($1, $2, $3, $4, $5, $6, $7, $8) returning id",
    )
    .bind(name)
    .bind(rust_decimal::Decimal::from(amount))
    .bind(ctx.pipeline)
    .bind(ctx.stage(stage))
    .bind(close)
    .bind(company)
    .bind(next_step)
    .bind(if amount >= 30000 { "high" } else { "medium" })
    .fetch_one(&mut *conn)
    .await?;
    for c in contacts {
        sqlx::query("insert into deal_contacts (deal_id, contact_id) values ($1, $2)")
            .bind(id)
            .bind(c)
            .execute(&mut *conn)
            .await?;
    }
    Ok(id)
}

#[allow(clippy::too_many_arguments)]
async fn activity(
    conn: &mut PgConnection,
    kind: &'static str,
    subject: Option<&str>,
    body: &str,
    direction: Option<&str>,
    participants: Value,
    hours_ago: i64,
    links: &[(&str, Uuid)],
) -> anyhow::Result<Uuid> {
    Ok(activities::insert(
        conn,
        NewActivity {
            kind,
            subject: subject.map(str::to_string),
            body: body.to_string(),
            direction: direction.map(str::to_string),
            participants: Some(participants),
            occurred_at: Some(Utc::now() - Duration::hours(hours_ago)),
            created_by: "connector",
            ..Default::default()
        },
        links,
    )
    .await?)
}

async fn task(
    conn: &mut PgConnection,
    title: &str,
    kind: &str,
    priority: &str,
    due_in_hours: i64,
    links: (Option<Uuid>, Option<Uuid>, Option<Uuid>),
    by_ai: bool,
) -> anyhow::Result<()> {
    sqlx::query(
        "insert into tasks (title, kind, priority, due_at, contact_id, company_id, deal_id, created_by)
         values ($1, $2, $3, $4, $5, $6, $7, $8)",
    )
    .bind(title)
    .bind(kind)
    .bind(priority)
    .bind(Utc::now() + Duration::hours(due_in_hours))
    .bind(links.0)
    .bind(links.1)
    .bind(links.2)
    .bind(if by_ai { "ai" } else { "user" })
    .execute(&mut *conn)
    .await?;
    Ok(())
}

pub async fn seed_demo(pool: &PgPool, verbose: bool) -> anyhow::Result<()> {
    let existing: i64 = sqlx::query_scalar("select count(*) from companies")
        .fetch_one(pool)
        .await?;
    if existing > 0 {
        if verbose {
            println!("There's already data in this database, so demo data was not added.");
        }
        return Ok(());
    }

    let pipeline: Uuid =
        sqlx::query_scalar("select id from pipelines order by position, created_at limit 1")
            .fetch_one(pool)
            .await?;
    let stages: Vec<(String, Uuid)> = sqlx::query_as(
        "select name, id from pipeline_stages where pipeline_id = $1 order by position",
    )
    .bind(pipeline)
    .fetch_all(pool)
    .await?;
    let ctx = Ctx { stages, pipeline };
    let me: String = sqlx::query_scalar(
        "select split_part(name, ' ', 1) from users order by created_at limit 1",
    )
    .fetch_optional(pool)
    .await?
    .unwrap_or_else(|| "Alex".into());

    let mut tx = pool.begin().await?;
    let c = &mut *tx;

    // Companies -----------------------------------------------------------------
    let ferrous = company(c, json!({"name": "Ferrous Labs", "domain": "ferrouslabs.io", "industry": "Climate fintech",
        "employees": 28, "city": "Manchester", "country": "United Kingdom", "lifecycle_stage": "opportunity",
        "description": "Carbon accounting platform for mid-market manufacturers. Seed funded, growing the sales team."})).await?;
    let brightwell = company(c, json!({"name": "Brightwell Health", "domain": "brightwellhealth.co.uk", "industry": "Healthtech",
        "employees": 64, "city": "Leeds", "country": "United Kingdom", "lifecycle_stage": "customer",
        "description": "Remote monitoring for community care teams. Works closely with NHS trusts."})).await?;
    let quayside = company(c, json!({"name": "Quayside Freight", "domain": "quaysidefreight.com", "industry": "Logistics",
        "employees": 140, "city": "Bristol", "country": "United Kingdom", "lifecycle_stage": "opportunity",
        "description": "Freight forwarding across the UK and EU ports."})).await?;
    let oakum = company(c, json!({"name": "Oakum Robotics", "domain": "oakumrobotics.com", "industry": "Robotics",
        "employees": 35, "city": "Cambridge", "country": "United Kingdom", "lifecycle_stage": "lead"})).await?;
    let tallow = company(
        c,
        json!({"name": "Tallow & Rye", "domain": "tallowandrye.co.uk", "industry": "Food and drink",
        "employees": 22, "city": "London", "country": "United Kingdom", "lifecycle_stage": "lead",
        "description": "Direct-to-consumer bakery brand, raising a Series A next year."}),
    )
    .await?;
    let larkspur = company(c, json!({"name": "Larkspur Legal", "domain": "larkspurlegal.co.uk", "industry": "Legal services",
        "employees": 48, "city": "Edinburgh", "country": "United Kingdom", "lifecycle_stage": "opportunity"})).await?;
    let copperline = company(c, json!({"name": "Copperline Energy", "domain": "copperline.energy", "industry": "Renewable energy",
        "employees": 90, "city": "Glasgow", "country": "United Kingdom", "lifecycle_stage": "customer",
        "description": "Community solar developer."})).await?;
    let hollis = company(c, json!({"name": "Hollis & Pike", "domain": "hollispike.studio", "industry": "Design agency",
        "employees": 12, "city": "Brighton", "country": "United Kingdom", "lifecycle_stage": "lead"})).await?;

    // Contacts --------------------------------------------------------------------
    let priya = contact(c, json!({"first_name": "Priya", "last_name": "Natarajan", "email": "priya@ferrouslabs.io",
        "job_title": "CEO and co-founder", "phone": "+44 161 555 0142", "company_id": ferrous, "lifecycle_stage": "opportunity", "source": "outlook"})).await?;
    let tom = contact(c, json!({"first_name": "Tom", "last_name": "Ashworth", "email": "tom@ferrouslabs.io",
        "job_title": "Head of Operations", "company_id": ferrous, "lifecycle_stage": "opportunity", "source": "outlook"})).await?;
    let helen = contact(c, json!({"first_name": "Helen", "last_name": "Okafor", "email": "helen.okafor@brightwellhealth.co.uk",
        "job_title": "Chief Operating Officer", "phone": "+44 113 555 0199", "company_id": brightwell, "lifecycle_stage": "customer", "source": "outlook"})).await?;
    let marcus = contact(c, json!({"first_name": "Marcus", "last_name": "Reid", "email": "marcus.reid@brightwellhealth.co.uk",
        "job_title": "CTO", "company_id": brightwell, "lifecycle_stage": "customer", "source": "teams"})).await?;
    let gareth = contact(c, json!({"first_name": "Gareth", "last_name": "Lewis", "email": "gareth.lewis@quaysidefreight.com",
        "job_title": "Operations Director", "phone": "+44 117 555 0123", "company_id": quayside, "lifecycle_stage": "opportunity", "source": "outlook"})).await?;
    let sian = contact(c, json!({"first_name": "Siân", "last_name": "Morgan", "email": "sian.morgan@quaysidefreight.com",
        "job_title": "Finance Director", "company_id": quayside, "lifecycle_stage": "opportunity", "source": "outlook"})).await?;
    let felix = contact(
        c,
        json!({"first_name": "Felix", "last_name": "Brandt", "email": "felix@oakumrobotics.com",
        "job_title": "Founder", "company_id": oakum, "lifecycle_stage": "lead", "source": "slack"}),
    )
    .await?;
    let amara = contact(c, json!({"first_name": "Amara", "last_name": "Osei", "email": "amara@tallowandrye.co.uk",
        "job_title": "Founder and CEO", "company_id": tallow, "lifecycle_stage": "lead", "source": "slack"})).await?;
    let callum = contact(c, json!({"first_name": "Callum", "last_name": "Fraser", "email": "c.fraser@larkspurlegal.co.uk",
        "job_title": "Managing Partner", "company_id": larkspur, "lifecycle_stage": "opportunity", "source": "teams"})).await?;
    let isla = contact(c, json!({"first_name": "Isla", "last_name": "McLeod", "email": "i.mcleod@larkspurlegal.co.uk",
        "job_title": "Head of Innovation", "company_id": larkspur, "lifecycle_stage": "opportunity", "source": "outlook"})).await?;
    let ewan = contact(c, json!({"first_name": "Ewan", "last_name": "Campbell", "email": "ewan.campbell@copperline.energy",
        "job_title": "Head of IT", "company_id": copperline, "lifecycle_stage": "customer", "source": "teams"})).await?;
    let rachel = contact(c, json!({"first_name": "Rachel", "last_name": "Duff", "email": "rachel.duff@copperline.energy",
        "job_title": "Procurement Lead", "company_id": copperline, "lifecycle_stage": "customer", "source": "outlook"})).await?;
    let jess = contact(c, json!({"first_name": "Jess", "last_name": "Hollis", "email": "jess@hollispike.studio",
        "job_title": "Co-founder", "company_id": hollis, "lifecycle_stage": "lead", "source": "manual"})).await?;

    // Deals -------------------------------------------------------------------------
    let d_ferrous = deal(
        c,
        &ctx,
        "Ferrous Labs: CRM setup and migration",
        14500,
        "Proposal sent",
        21,
        ferrous,
        &[priya, tom],
        Some("Wait for Priya's feedback on the proposal"),
    )
    .await?;
    let d_bw_audit = deal(
        c,
        &ctx,
        "Brightwell Health: Data sovereignty audit",
        9800,
        "Closed won",
        -12,
        brightwell,
        &[helen],
        None,
    )
    .await?;
    let d_bw_ai = deal(
        c,
        &ctx,
        "Brightwell Health: Agentic AI triage pilot",
        32000,
        "Discovery",
        60,
        brightwell,
        &[helen, marcus],
        Some("Scope the pilot with Marcus"),
    )
    .await?;
    let d_quay = deal(
        c,
        &ctx,
        "Quayside Freight: Sovereign cloud migration",
        68000,
        "Negotiation",
        14,
        quayside,
        &[gareth, sian],
        Some("Revised timeline to Gareth"),
    )
    .await?;
    let d_lark = deal(
        c,
        &ctx,
        "Larkspur Legal: Document AI implementation",
        24000,
        "Qualified",
        35,
        larkspur,
        &[callum, isla],
        Some("Send proposal"),
    )
    .await?;
    let _d_copper_ret = deal(
        c,
        &ctx,
        "Copperline Energy: Commercial advisory retainer",
        18000,
        "Closed won",
        -20,
        copperline,
        &[rachel],
        None,
    )
    .await?;
    let d_copper_res = deal(
        c,
        &ctx,
        "Copperline Energy: Data residency review",
        7500,
        "Proposal sent",
        18,
        copperline,
        &[ewan],
        None,
    )
    .await?;
    let d_oakum = deal(
        c,
        &ctx,
        "Oakum Robotics: Sales process and CRM design",
        6000,
        "Discovery",
        40,
        oakum,
        &[felix],
        None,
    )
    .await?;
    let _d_hollis = deal(
        c,
        &ctx,
        "Hollis & Pike: Analytics consolidation",
        3200,
        "Closed lost",
        -30,
        hollis,
        &[jess],
        None,
    )
    .await?;

    // Timeline ------------------------------------------------------------------------
    let ferrous_proposal_email = activity(c, "email", Some("CRM setup proposal for Ferrous Labs"),
        &format!("Hi Priya,\n\nAs promised, here's the proposal for getting your CRM set up properly before the new sales hires start. It covers the data model, migrating the spreadsheets Tom showed me, and two training sessions for the team.\n\nHappy to walk through it on a call if that's easier.\n\nBest,\n{me}"),
        Some("outbound"), json!(["priya@ferrouslabs.io", "tom@ferrouslabs.io"]), 24 * 6, &[("contact", priya), ("contact", tom), ("deal", d_ferrous)]).await?;
    activity(c, "meeting", Some("Ferrous Labs discovery call"),
        "Priya and Tom joined. Sales team growing from 2 to 6 in Q1. Pipeline lives in three spreadsheets and a shared inbox. Priya wants a single view of investor and customer conversations. Budget signed off up to £15k.",
        None, json!(["Priya Natarajan", "Tom Ashworth"]), 24 * 9, &[("contact", priya), ("contact", tom), ("deal", d_ferrous)]).await?;
    activity(c, "slack", None, "Tom: we've pulled together the spreadsheets, shared folder link is in the channel. Shout if anything's missing.",
        Some("inbound"), json!(["Tom Ashworth"]), 24 * 7, &[("contact", tom), ("deal", d_ferrous)]).await?;

    activity(c, "email", Some("Audit report: final version"),
        "Hi Helen,\n\nAttached is the final audit report with the remediation plan we agreed. The two high-priority items are the backup location and the analytics vendor's sub-processors.",
        Some("outbound"), json!(["helen.okafor@brightwellhealth.co.uk"]), 24 * 13, &[("contact", helen), ("deal", d_bw_audit)]).await?;
    let bw_meeting = activity(c, "meeting", Some("AI triage pilot: discovery call"),
        "Transcript (excerpt)\n\nMarcus: The nurses spend the first hour of every shift reading overnight alerts. Most are noise.\nHelen: If we can cut that in half without anything leaving our Azure tenant, the board will fund a pilot.\nMarcus: We'd want to start with one trust, probably Wakefield, and run it alongside the current process for six weeks.\nHelen: Can you send us a rough plan before the board meeting on the 28th?",
        None, json!(["Helen Okafor", "Marcus Reid"]), 26, &[("contact", helen), ("contact", marcus), ("deal", d_bw_ai)]).await?;

    activity(c, "email", Some("Re: Migration plan"),
        "Gareth: Thanks for the plan. Siân's happy with the commercials. The only sticking point is the cutover in December, that's our busiest month. Could we look at January?",
        Some("inbound"), json!(["gareth.lewis@quaysidefreight.com"]), 30, &[("contact", gareth), ("deal", d_quay)]).await?;
    let quay_meeting = activity(c, "meeting", Some("Quayside weekly check-in"),
        "Transcript (excerpt)\n\nGareth: So if cutover moves to the second week of January, does the price change?\nYou: No, the price holds. I'll send a revised timeline showing the January cutover.\nGareth: Great, if I have that by Friday I can take it to the board on Monday.\nNadia: And I'll need the network diagram for the EU region by then too.",
        None, json!(["Gareth Lewis", "Nadia Kowalski", "Siân Morgan"]), 5, &[("contact", gareth), ("deal", d_quay)]).await?;
    let nadia_email = activity(c, "email", Some("Re: Migration plan"),
        "Hi both,\n\nJumping in from the IT side. I'll be the technical owner for the migration on our end, so please loop me into anything on networking or identity.\n\nThanks,\nNadia Kowalski\nHead of IT Infrastructure | Quayside Freight\n+44 117 555 0188\nnadia.kowalski@quaysidefreight.com",
        Some("inbound"), json!(["nadia.kowalski@quaysidefreight.com", "gareth.lewis@quaysidefreight.com"]), 4, &[("deal", d_quay)]).await?;

    let callum_teams = activity(c, "teams", None,
        "Callum: Got the proposal, thank you. The partners are reviewing it on Thursday. First impressions are good, the fixed price helps a lot.",
        Some("inbound"), json!(["Callum Fraser"]), 3, &[("contact", callum)]).await?;
    let isla_email = activity(c, "email", Some("Where would the document AI run?"),
        "Hi,\n\nBefore Thursday's partner meeting, could you confirm where the document AI would actually run? Our clients will ask whether their files ever leave the UK, and I'd like to give the partners a straight answer.\n\nThanks,\nIsla",
        Some("inbound"), json!(["i.mcleod@larkspurlegal.co.uk"]), 2, &[("contact", isla), ("deal", d_lark)]).await?;

    activity(c, "teams", None, "Ewan: Quick heads up, we completed the Solway acquisition last week so headcount is now about 120. Might affect the residency review scope.",
        Some("inbound"), json!(["Ewan Campbell"]), 20, &[("contact", ewan), ("company", copperline)]).await?;
    let ewan_teams = activity(c, "teams", None, "Ewan: Also, can we push the residency review kick-off to after the 3rd? Procurement need to update the PO.",
        Some("inbound"), json!(["Ewan Campbell"]), 19, &[("contact", ewan), ("deal", d_copper_res)]).await?;

    let amara_slack = activity(c, "slack", None,
        "Amara: Hey! We're getting serious about the Series A prep. Investors keep asking for pipeline numbers and ours live in my head and a Notion page. Could you scope getting a proper CRM in? Ideally before February.",
        Some("inbound"), json!(["Amara Osei"]), 8, &[("contact", amara)]).await?;
    let felix_slack = activity(c, "slack", None,
        "Felix: Intro for you: Marlow Bay Analytics (marlowbay.ai) are building GDPR-compliant data pipelines for retailers and need help with their EU hosting. Their CTO is Dan Whitlock. Happy to make the connection.",
        Some("inbound"), json!(["Felix Brandt"]), 6, &[("contact", felix)]).await?;
    let tom_email = activity(c, "email", Some("Spreadsheet access"),
        "Hi,\n\nI've given you edit access to the pipeline sheets.\n\nCheers,\nTom Ashworth\nChief Operating Officer | Ferrous Labs",
        Some("inbound"), json!(["tom@ferrouslabs.io"]), 28, &[("contact", tom)]).await?;

    activity(
        c,
        "note",
        None,
        "Jess said they're going with an in-house build for now. Worth checking back in spring.",
        None,
        json!([]),
        24 * 30,
        &[("contact", jess), ("company", hollis)],
    )
    .await?;

    // Tasks -----------------------------------------------------------------------------
    task(
        c,
        "Send Brightwell the pilot plan outline",
        "email",
        "high",
        -20,
        (Some(helen), Some(brightwell), Some(d_bw_ai)),
        true,
    )
    .await?;
    task(
        c,
        "Call Felix about the CRM design scope",
        "call",
        "medium",
        3,
        (Some(felix), Some(oakum), Some(d_oakum)),
        false,
    )
    .await?;
    task(
        c,
        "Prepare Larkspur partner meeting notes",
        "todo",
        "medium",
        50,
        (Some(callum), Some(larkspur), Some(d_lark)),
        false,
    )
    .await?;
    task(
        c,
        "Check in with Jess at Hollis & Pike",
        "follow_up",
        "low",
        24 * 120,
        (Some(jess), Some(hollis), None),
        false,
    )
    .await?;

    // Suggestions waiting for review ------------------------------------------------------
    let friday = {
        let today = Utc::now().date_naive();
        let mut d = today;
        while d.weekday() != Weekday::Fri || d == today {
            d += Duration::days(1);
        }
        d.and_hms_opt(12, 0, 0).unwrap().and_utc()
    };

    let p = |kind: &str,
             title: &str,
             rationale: &str,
             payload: Value,
             target: Option<(&str, Uuid)>,
             conf: f32,
             channel: &str,
             excerpt: &str,
             source: Option<Uuid>| NewProposal {
        kind: kind.into(),
        title: title.into(),
        rationale: Some(rationale.into()),
        payload,
        target_type: target.map(|t| t.0.to_string()),
        target_id: target.map(|t| t.1),
        confidence: Some(conf),
        source_channel: Some(channel.into()),
        source_excerpt: Some(excerpt.into()),
        source_activity_id: source,
    };

    let items = vec![
        p("send_email", "Follow up with Priya on the CRM proposal",
          "The proposal went to Priya 6 days ago with no reply. Her close date is in 3 weeks and the new sales hires start in January.",
          json!({"to": ["priya@ferrouslabs.io"], "cc": ["tom@ferrouslabs.io"], "subject": "Re: CRM setup proposal for Ferrous Labs",
                 "body": format!("Hi Priya,\n\nJust checking the proposal landed okay. I know January is creeping up, so if it helps I can hold two weeks in December to get the data model and migration done before the new team starts.\n\nWould a 20 minute call this week be useful to go through any questions?\n\nBest,\n{me}"),
                 "contact_id": priya, "deal_id": d_ferrous}),
          Some(("contact", priya)), 0.88, "outlook", "No reply to: CRM setup proposal for Ferrous Labs (sent 6 days ago)", Some(ferrous_proposal_email)),
        p("create_contact", "Add Nadia Kowalski from Quayside Freight",
          "New person on the Quayside migration thread. Her signature gives her role, phone and email, and she says she's the technical owner.",
          json!({"first_name": "Nadia", "last_name": "Kowalski", "email": "nadia.kowalski@quaysidefreight.com",
                 "job_title": "Head of IT Infrastructure", "phone": "+44 117 555 0188", "company_domain": "quaysidefreight.com",
                 "lifecycle_stage": "opportunity"}),
          None, 0.95, "outlook", "I'll be the technical owner for the migration on our end", Some(nadia_email)),
        p("create_task", "Send Quayside the revised January timeline by Friday",
          "In the weekly check-in you promised Gareth a revised timeline with a January cutover. He needs it by Friday for Monday's board.",
          json!({"title": "Send Gareth the revised migration timeline (January cutover) and Nadia the EU network diagram",
                 "kind": "email", "priority": "high", "due_at": friday.to_rfc3339(), "contact_id": gareth, "company_id": quayside}),
          Some(("deal", d_quay)), 0.91, "teams", "if I have that by Friday I can take it to the board on Monday", Some(quay_meeting)),
        p("update_deal", "Move Larkspur's Document AI deal to Proposal sent",
          "Callum confirmed on Teams that they've received the proposal and the partners review it on Thursday.",
          json!({"stage_id": ctx.stage("Proposal sent"), "next_step": "Partners review the proposal on Thursday"}),
          Some(("deal", d_lark)), 0.9, "teams", "Got the proposal, thank you. The partners are reviewing it on Thursday.", Some(callum_teams)),
        p("send_email", "Answer Isla's question about where the AI runs",
          "Isla asked a direct question ahead of Thursday's partner meeting. A clear answer now should help the deal through.",
          json!({"to": ["i.mcleod@larkspurlegal.co.uk"], "subject": "Re: Where would the document AI run?",
                 "body": format!("Hi Isla,\n\nShort answer: everything runs in the UK. Documents are processed and stored in your own Azure tenant in the UK South region, and nothing is used to train any model.\n\nI'll put a one page summary together that the partners can share with clients, and bring it on Thursday.\n\nBest,\n{me}"),
                 "contact_id": isla, "deal_id": d_lark}),
          Some(("contact", isla)), 0.82, "outlook", "could you confirm where the document AI would actually run?", Some(isla_email)),
        p("add_note", "Summarise the Brightwell AI triage discovery call",
          "Meeting transcript with Helen and Marcus. Key requirements and a deadline came up.",
          json!({"subject": "Discovery call summary", "body": "Problem: nurses spend the first hour of each shift reading overnight alerts, most are noise.\nGoal: cut that time by half.\nConstraint: nothing leaves Brightwell's Azure tenant.\nPilot: one trust (likely Wakefield), run in parallel with the current process for six weeks.\nNext: rough plan needed before the board meeting on the 28th.",
                 "links": [{"object_type": "contact", "object_id": helen}, {"object_type": "contact", "object_id": marcus}]}),
          Some(("deal", d_bw_ai)), 0.93, "teams", "Can you send us a rough plan before the board meeting on the 28th?", Some(bw_meeting)),
        p("create_deal", "Create a deal for Tallow & Rye's CRM setup",
          "Amara asked you to scope a CRM before their Series A, with a February deadline.",
          json!({"name": "Tallow & Rye: CRM setup before Series A", "amount": 8500, "stage_name": "Discovery",
                 "company_id": tallow, "contact_ids": [amara], "close_date": (Utc::now() + Duration::days(45)).date_naive().to_string(),
                 "next_step": "Scoping call with Amara"}),
          None, 0.84, "slack", "Could you scope getting a proper CRM in? Ideally before February.", Some(amara_slack)),
        p("create_company", "Add Marlow Bay Analytics as a lead",
          "Felix offered an introduction to a company that needs help with EU hosting, which matches your services.",
          json!({"name": "Marlow Bay Analytics", "domain": "marlowbay.ai", "industry": "Data and analytics",
                 "lifecycle_stage": "lead", "description": "Builds GDPR-compliant data pipelines for retailers. Needs help with EU hosting. Introduced by Felix Brandt (Oakum Robotics); CTO is Dan Whitlock."}),
          None, 0.76, "slack", "Marlow Bay Analytics (marlowbay.ai) are building GDPR-compliant data pipelines", Some(felix_slack)),
        p("update_contact", "Update Tom Ashworth's job title",
          "Tom's email signature now says Chief Operating Officer.",
          json!({"job_title": "Chief Operating Officer"}),
          Some(("contact", tom)), 0.97, "outlook", "Tom Ashworth, Chief Operating Officer | Ferrous Labs", Some(tom_email)),
        p("update_company", "Update Copperline's headcount after the Solway acquisition",
          "Ewan said headcount is now about 120 following an acquisition.",
          json!({"employees": 120, "description": "Community solar developer. Acquired Solway in October."}),
          Some(("company", copperline)), 0.8, "teams", "we completed the Solway acquisition last week so headcount is now about 120", None),
        p("update_deal", "Push the Copperline residency review close date",
          "Ewan asked to move the kick-off to after the 3rd while procurement update the PO.",
          json!({"close_date": (Utc::now() + Duration::days(32)).date_naive().to_string(), "next_step": "Wait for updated PO from procurement"}),
          Some(("deal", d_copper_res)), 0.71, "teams", "can we push the residency review kick-off to after the 3rd?", Some(ewan_teams)),
    ];
    for item in items {
        proposals::insert(c, item).await?;
    }

    // A little history, so the Decided tab isn't empty.
    sqlx::query(
        "insert into proposals (kind, status, title, rationale, payload, confidence, source_channel, source_excerpt, feedback, decided_at, created_at)
         values ('create_contact', 'rejected', 'Add noreply@calendly.com as a contact', 'New sender on a meeting confirmation.',
                 '{\"email\": \"noreply@calendly.com\"}', 0.41, 'outlook', 'Your meeting has been scheduled',
                 'Automated sender, ignore these.', now() - interval '2 days', now() - interval '2 days')",
    )
    .execute(&mut *c)
    .await?;

    // Give everything to the first user, if one exists yet. Otherwise the first account created gets it.
    for table in ["companies", "contacts", "deals", "tasks"] {
        sqlx::query(&format!(
            "update {table} set owner_id = (select id from users order by created_at limit 1) where owner_id is null"
        ))
        .execute(&mut *c)
        .await?;
    }

    tx.commit().await?;
    tracing::info!("demo data added");
    Ok(())
}
