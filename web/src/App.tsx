import { useEffect, useState } from "react";
import { BrowserRouter, Navigate, Route, Routes } from "react-router-dom";
import { useQueryClient } from "@tanstack/react-query";
import { get, post, setUnauthorizedHandler, type User } from "./lib/api";
import { Layout } from "./components/Layout";
import { Loading } from "./components/ui";
import { ReviewPage } from "./pages/Review";
import { CompaniesPage, ContactsPage } from "./pages/Lists";
import { DealsPage } from "./pages/Deals";
import { TasksPage } from "./pages/Tasks";
import { OutboxPage } from "./pages/Outbox";
import { ConnectionsPage } from "./pages/Connections";
import { CompanyRecord, ContactRecord, DealRecord } from "./pages/Records";
import { LoginPage } from "./pages/Login";

type AuthState = { status: "loading" } | { status: "signed-out"; needsSetup: boolean } | { status: "signed-in"; user: User };

export function App() {
  const [auth, setAuth] = useState<AuthState>({ status: "loading" });
  const qc = useQueryClient();

  const refresh = () =>
    get<{ needs_setup: boolean; user: User | null }>("/auth/status")
      .then((s) => setAuth(s.user ? { status: "signed-in", user: s.user } : { status: "signed-out", needsSetup: s.needs_setup }))
      .catch(() => setAuth({ status: "signed-out", needsSetup: false }));

  useEffect(() => {
    refresh();
    setUnauthorizedHandler(() => {
      qc.clear();
      refresh();
    });
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  if (auth.status === "loading") return <Loading label="Starting sAIles" />;
  if (auth.status === "signed-out") {
    return <LoginPage needsSetup={auth.needsSetup} onSignedIn={(user) => setAuth({ status: "signed-in", user })} />;
  }

  const signOut = async () => {
    await post("/auth/logout").catch(() => {});
    qc.clear();
    setAuth({ status: "signed-out", needsSetup: false });
  };

  return (
    <BrowserRouter>
      <Routes>
        <Route element={<Layout user={auth.user} onSignOut={signOut} />}>
          <Route index element={<ReviewPage />} />
          <Route path="contacts" element={<ContactsPage />} />
          <Route path="contacts/:id" element={<ContactRecord />} />
          <Route path="companies" element={<CompaniesPage />} />
          <Route path="companies/:id" element={<CompanyRecord />} />
          <Route path="deals" element={<DealsPage />} />
          <Route path="deals/:id" element={<DealRecord />} />
          <Route path="tasks" element={<TasksPage />} />
          <Route path="outbox" element={<OutboxPage />} />
          <Route path="connections" element={<ConnectionsPage />} />
          <Route path="*" element={<Navigate to="/" replace />} />
        </Route>
      </Routes>
    </BrowserRouter>
  );
}
