import { useEffect, useState, type ReactNode } from "react";
import { useNavigate } from "react-router-dom";
import { keepPreviousData, useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { ArrowDown, ArrowUp, ChevronLeft, ChevronRight, Search, Trash2 } from "lucide-react";
import { del, get, qs, type Page } from "../lib/api";
import { Empty, Loading, useToast } from "./ui";

export interface Column<T> {
  key: string;
  label: string;
  sort?: string;
  render: (row: T) => ReactNode;
  className?: string;
}

export interface View {
  key: string;
  label: string;
  params?: Record<string, string>;
}

const PAGE_SIZE = 50;

export function ObjectTable<T extends { id: string }>({
  path,
  noun,
  views,
  columns,
  rowHref,
  emptyAction,
  extraParams,
  toolbarExtra,
}: {
  path: string;
  noun: { one: string; many: string };
  views: View[];
  columns: Column<T>[];
  rowHref: (row: T) => string;
  emptyAction?: ReactNode;
  extraParams?: Record<string, string>;
  toolbarExtra?: ReactNode;
}) {
  const [view, setView] = useState(views[0].key);
  const [search, setSearch] = useState("");
  const [q, setQ] = useState("");
  const [sort, setSort] = useState<{ key: string; dir: "asc" | "desc" } | null>(null);
  const [offset, setOffset] = useState(0);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const navigate = useNavigate();
  const qc = useQueryClient();
  const toast = useToast();

  useEffect(() => {
    const t = setTimeout(() => {
      setQ(search.trim());
      setOffset(0);
    }, 200);
    return () => clearTimeout(t);
  }, [search]);

  const viewParams = views.find((v) => v.key === view)?.params ?? {};
  const params = {
    ...viewParams,
    ...extraParams,
    q,
    sort: sort?.key,
    dir: sort?.dir,
    limit: PAGE_SIZE,
    offset,
  };
  const list = useQuery({
    queryKey: [path.slice(1), "list", params],
    queryFn: () => get<Page<T>>(`${path}${qs(params)}`),
    placeholderData: keepPreviousData,
  });

  const remove = useMutation({
    mutationFn: async (ids: string[]) => {
      for (const id of ids) await del(`${path}/${id}`);
      return ids.length;
    },
    onSuccess: (n) => {
      toast(`Deleted ${n} ${n === 1 ? noun.one : noun.many}`);
      setSelected(new Set());
      qc.invalidateQueries();
    },
    onError: (e: Error) => toast(e.message, true),
  });

  const items = list.data?.items ?? [];
  const total = list.data?.total ?? 0;
  const allSelected = items.length > 0 && items.every((i) => selected.has(i.id));

  const toggleSort = (key: string) => {
    setOffset(0);
    setSort((s) => (s?.key !== key ? { key, dir: "asc" } : s.dir === "asc" ? { key, dir: "desc" } : null));
  };

  return (
    <>
      <div className="tabs" role="tablist">
        {views.map((v) => (
          <button
            key={v.key}
            role="tab"
            aria-selected={view === v.key}
            className={view === v.key ? "active" : ""}
            onClick={() => {
              setView(v.key);
              setOffset(0);
              setSelected(new Set());
            }}
          >
            {v.label}
          </button>
        ))}
      </div>
      <div className="table-card">
        <div className="table-toolbar">
          <div className="search">
            <Search size={15} />
            <input
              type="search"
              placeholder={`Search ${noun.many}`}
              aria-label={`Search ${noun.many}`}
              value={search}
              onChange={(e) => setSearch(e.target.value)}
            />
          </div>
          {selected.size > 0 && (
            <>
              <span className="muted">{selected.size} selected</span>
              <button
                className="btn small danger"
                disabled={remove.isPending}
                onClick={() => {
                  if (confirm(`Delete ${selected.size} ${selected.size === 1 ? noun.one : noun.many}? This can't be undone.`)) {
                    remove.mutate([...selected]);
                  }
                }}
              >
                <Trash2 size={14} /> Delete
              </button>
            </>
          )}
          <div style={{ marginLeft: "auto", display: "flex", gap: 8 }}>{toolbarExtra}</div>
        </div>
        {list.isLoading ? (
          <Loading />
        ) : items.length === 0 ? (
          <Empty title={q ? `No ${noun.many} match “${q}”` : `No ${noun.many} yet`} action={!q ? emptyAction : undefined}>
            {q ? "Try a different search." : `${noun.many[0].toUpperCase() + noun.many.slice(1)} you add, or approve from Review, appear here.`}
          </Empty>
        ) : (
          <div className="table-scroll">
            <table className="data">
              <thead>
                <tr>
                  <th className="check">
                    <input
                      type="checkbox"
                      aria-label="Select all on this page"
                      checked={allSelected}
                      onChange={(e) => setSelected(e.target.checked ? new Set(items.map((i) => i.id)) : new Set())}
                    />
                  </th>
                  {columns.map((c) => (
                    <th
                      key={c.key}
                      className={c.sort ? "sortable" : ""}
                      onClick={c.sort ? () => toggleSort(c.sort!) : undefined}
                      aria-sort={sort?.key === c.sort ? (sort?.dir === "asc" ? "ascending" : "descending") : undefined}
                    >
                      <span style={{ display: "inline-flex", alignItems: "center", gap: 4 }}>
                        {c.label}
                        {c.sort && sort?.key === c.sort && (sort.dir === "asc" ? <ArrowUp size={13} /> : <ArrowDown size={13} />)}
                      </span>
                    </th>
                  ))}
                </tr>
              </thead>
              <tbody>
                {items.map((row) => (
                  <tr
                    key={row.id}
                    onClick={(e) => {
                      if ((e.target as HTMLElement).closest("input, a, button")) return;
                      navigate(rowHref(row));
                    }}
                    style={{ cursor: "pointer" }}
                  >
                    <td className="check">
                      <input
                        type="checkbox"
                        aria-label="Select row"
                        checked={selected.has(row.id)}
                        onChange={(e) =>
                          setSelected((s) => {
                            const n = new Set(s);
                            if (e.target.checked) n.add(row.id);
                            else n.delete(row.id);
                            return n;
                          })
                        }
                      />
                    </td>
                    {columns.map((c) => (
                      <td key={c.key} className={c.className}>
                        {c.render(row)}
                      </td>
                    ))}
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
        {total > 0 && (
          <div className="pager">
            <span className="num">
              {offset + 1}–{Math.min(offset + PAGE_SIZE, total)} of {total}
            </span>
            <button className="btn small icon" aria-label="Previous page" disabled={offset === 0} onClick={() => setOffset(Math.max(0, offset - PAGE_SIZE))}>
              <ChevronLeft size={16} />
            </button>
            <button className="btn small icon" aria-label="Next page" disabled={offset + PAGE_SIZE >= total} onClick={() => setOffset(offset + PAGE_SIZE)}>
              <ChevronRight size={16} />
            </button>
          </div>
        )}
      </div>
    </>
  );
}
