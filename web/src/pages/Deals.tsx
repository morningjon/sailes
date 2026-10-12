import { useMemo, useState } from "react";
import { Link, useNavigate } from "react-router-dom";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Columns3, Plus, Rows3 } from "lucide-react";
import { get, patch, type Deal, type Page } from "../lib/api";
import { formatDate, formatMoney } from "../lib/format";
import { usePipelines } from "../lib/hooks";
import { ObjectTable } from "../components/ObjectTable";
import { CreateDealPanel } from "../components/forms";
import { Loading, StagePill, useToast } from "../components/ui";

function Board({ pipelineId }: { pipelineId: string }) {
  const pipelines = usePipelines();
  const pipeline = pipelines.data?.find((p) => p.id === pipelineId);
  const deals = useQuery({
    queryKey: ["deals", "board", pipelineId],
    queryFn: () => get<Page<Deal>>(`/deals?pipeline_id=${pipelineId}&limit=1000&sort=close_date&dir=asc`),
  });
  const qc = useQueryClient();
  const toast = useToast();
  const [dragging, setDragging] = useState<string | null>(null);
  const [over, setOver] = useState<string | null>(null);

  const move = useMutation({
    mutationFn: ({ id, stage_id }: { id: string; stage_id: string }) => patch<Deal>(`/deals/${id}`, { stage_id }),
    onMutate: async ({ id, stage_id }) => {
      const key = ["deals", "board", pipelineId];
      await qc.cancelQueries({ queryKey: key });
      const prev = qc.getQueryData<Page<Deal>>(key);
      const stage = pipeline?.stages.find((s) => s.id === stage_id);
      if (prev && stage) {
        qc.setQueryData<Page<Deal>>(key, {
          ...prev,
          items: prev.items.map((d) =>
            d.id === id ? { ...d, stage_id, stage_name: stage.name, is_closed: stage.is_closed, is_won: stage.is_won } : d,
          ),
        });
      }
      return { prev };
    },
    onError: (e: Error, _v, ctx) => {
      if (ctx?.prev) qc.setQueryData(["deals", "board", pipelineId], ctx.prev);
      toast(e.message, true);
    },
    onSuccess: (d) => toast(`Moved to ${d.stage_name}`),
    onSettled: () => qc.invalidateQueries(),
  });

  const byStage = useMemo(() => {
    const m = new Map<string, Deal[]>();
    for (const d of deals.data?.items ?? []) m.set(d.stage_id, [...(m.get(d.stage_id) ?? []), d]);
    return m;
  }, [deals.data]);

  if (!pipeline || deals.isLoading) return <Loading />;

  return (
    <div className="board">
      {pipeline.stages.map((stage) => {
        const items = byStage.get(stage.id) ?? [];
        const total = items.reduce((sum, d) => sum + (d.amount ?? 0), 0);
        return (
          <div
            key={stage.id}
            className={`column ${over === stage.id ? "drop" : ""}`}
            onDragOver={(e) => {
              e.preventDefault();
              setOver(stage.id);
            }}
            onDragLeave={() => setOver((o) => (o === stage.id ? null : o))}
            onDrop={(e) => {
              e.preventDefault();
              setOver(null);
              const id = e.dataTransfer.getData("text/plain");
              const deal = deals.data?.items.find((d) => d.id === id);
              if (deal && deal.stage_id !== stage.id) move.mutate({ id, stage_id: stage.id });
            }}
          >
            <div className="column-head">
              <div className="name">
                <span>{stage.name}</span>
                <span className="faint num">{items.length}</span>
              </div>
              <div className="total num">
                {formatMoney(total)}
                {!stage.is_closed && <span className="faint"> at {stage.probability}%</span>}
              </div>
            </div>
            <div className="column-cards">
              {items.map((d) => (
                <div
                  key={d.id}
                  className={`deal-card ${dragging === d.id ? "dragging" : ""}`}
                  draggable
                  onDragStart={(e) => {
                    e.dataTransfer.setData("text/plain", d.id);
                    e.dataTransfer.effectAllowed = "move";
                    setDragging(d.id);
                  }}
                  onDragEnd={() => setDragging(null)}
                >
                  <Link className="name" to={`/deals/${d.id}`}>
                    {d.name}
                  </Link>
                  <div className="amt num">{formatMoney(d.amount, d.currency)}</div>
                  <div className="meta">
                    <span>{d.company_name ?? ""}</span>
                    {d.close_date && <span className="num">{formatDate(d.close_date)}</span>}
                  </div>
                </div>
              ))}
            </div>
          </div>
        );
      })}
    </div>
  );
}

export function DealsPage() {
  const pipelines = usePipelines();
  const [mode, setMode] = useState<"board" | "table">(() => {
    try {
      return (localStorage.getItem("sailes.deals.mode") as "board" | "table") || "board";
    } catch {
      return "board";
    }
  });
  const [creating, setCreating] = useState(false);
  const navigate = useNavigate();
  const pipelineId = pipelines.data?.[0]?.id;

  const switchMode = (m: "board" | "table") => {
    setMode(m);
    try {
      localStorage.setItem("sailes.deals.mode", m);
    } catch {
      /* storage unavailable */
    }
  };

  const create = (
    <button className="btn primary" onClick={() => setCreating(true)}>
      <Plus size={16} /> Create deal
    </button>
  );

  return (
    <div className="page" style={{ maxWidth: "none" }}>
      <div className="page-head">
        <h1>Deals</h1>
        <div className="actions">
          <div style={{ display: "flex" }} role="group" aria-label="View">
            <button
              className={`btn ${mode === "board" ? "primary" : ""}`}
              style={{ borderTopRightRadius: 0, borderBottomRightRadius: 0 }}
              aria-pressed={mode === "board"}
              onClick={() => switchMode("board")}
            >
              <Columns3 size={16} /> Board
            </button>
            <button
              className={`btn ${mode === "table" ? "primary" : ""}`}
              style={{ borderTopLeftRadius: 0, borderBottomLeftRadius: 0, marginLeft: -1 }}
              aria-pressed={mode === "table"}
              onClick={() => switchMode("table")}
            >
              <Rows3 size={16} /> Table
            </button>
          </div>
          {create}
        </div>
      </div>
      {mode === "board" ? (
        pipelineId ? (
          <Board pipelineId={pipelineId} />
        ) : (
          <Loading />
        )
      ) : (
        <ObjectTable<Deal>
          path="/deals"
          noun={{ one: "deal", many: "deals" }}
          views={[
            { key: "open", label: "Open deals", params: { status: "open" } },
            { key: "all", label: "All deals" },
            { key: "won", label: "Won", params: { status: "won" } },
            { key: "lost", label: "Lost", params: { status: "lost" } },
          ]}
          rowHref={(d) => `/deals/${d.id}`}
          emptyAction={create}
          columns={[
            { key: "name", label: "Deal name", sort: "name", render: (d) => <Link to={`/deals/${d.id}`} style={{ fontWeight: 500 }}>{d.name}</Link> },
            { key: "stage", label: "Deal stage", sort: "stage", render: (d) => <StagePill name={d.stage_name} isClosed={d.is_closed} isWon={d.is_won} /> },
            { key: "amount", label: "Amount", sort: "amount", render: (d) => <span className="num">{formatMoney(d.amount, d.currency)}</span> },
            { key: "close", label: "Close date", sort: "close_date", render: (d) => <span className="num">{formatDate(d.close_date)}</span> },
            {
              key: "company",
              label: "Company",
              sort: "company",
              render: (d) => (d.company_id ? <Link to={`/companies/${d.company_id}`}>{d.company_name}</Link> : ""),
            },
            { key: "contacts", label: "Contacts", render: (d) => <span className="muted">{d.contact_names.join(", ")}</span> },
          ]}
        />
      )}
      {creating && <CreateDealPanel onClose={() => setCreating(false)} onCreated={(d) => navigate(`/deals/${d.id}`)} />}
    </div>
  );
}
