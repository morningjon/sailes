import { useQuery } from "@tanstack/react-query";
import { get, type Company, type Contact, type Page, type Pipeline, type Summary } from "./api";

export function usePipelines() {
  return useQuery({ queryKey: ["pipelines"], queryFn: () => get<Pipeline[]>("/pipelines"), staleTime: 60_000 });
}

export function useCompanyOptions() {
  return useQuery({
    queryKey: ["companies", "options"],
    queryFn: () => get<Page<Company>>("/companies?limit=1000&sort=name&dir=asc"),
    select: (p) => p.items.map((c) => ({ value: c.id, label: c.name })),
    staleTime: 30_000,
  });
}

export function useContactOptions() {
  return useQuery({
    queryKey: ["contacts", "options"],
    queryFn: () => get<Page<Contact>>("/contacts?limit=1000&sort=name&dir=asc"),
    select: (p) =>
      p.items.map((c) => ({
        value: c.id,
        label: `${c.first_name} ${c.last_name}`.trim() || c.email || "Unnamed",
        sub: c.company_name ?? c.email ?? "",
      })),
    staleTime: 30_000,
  });
}

export function useSummary() {
  return useQuery({ queryKey: ["summary"], queryFn: () => get<Summary>("/dashboard"), refetchInterval: 60_000 });
}
