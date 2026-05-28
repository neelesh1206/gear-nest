"use client";

import Link from "next/link";
import { useRouter } from "next/navigation";
import { useEffect, useMemo, useRef, useState, useTransition } from "react";
import { searchSuggestions, type SearchSuggestion } from "@/components/search/actions";
import { cn, debounce } from "@/lib/utils";

export function SearchBar({ initialQuery = "" }: { initialQuery?: string }) {
  const router = useRouter();
  const [value, setValue] = useState(initialQuery);
  const [suggestions, setSuggestions] = useState<SearchSuggestion[]>([]);
  const [open, setOpen] = useState(false);
  const [pending, startTransition] = useTransition();
  const containerRef = useRef<HTMLDivElement | null>(null);

  const runSearch = useMemo(
    () =>
      debounce((q: string) => {
        if (q.trim().length < 2) {
          setSuggestions([]);
          setOpen(false);
          return;
        }
        startTransition(async () => {
          const next = await searchSuggestions(q);
          setSuggestions(next);
          setOpen(next.length > 0);
        });
      }, 220),
    [],
  );

  useEffect(() => {
    runSearch(value);
  }, [value, runSearch]);

  useEffect(() => {
    function onClick(e: MouseEvent) {
      if (!containerRef.current?.contains(e.target as Node)) setOpen(false);
    }
    document.addEventListener("click", onClick);
    return () => document.removeEventListener("click", onClick);
  }, []);

  const submit = (e: React.FormEvent<HTMLFormElement>) => {
    e.preventDefault();
    const q = value.trim();
    setOpen(false);
    router.push(q ? `/?q=${encodeURIComponent(q)}` : "/");
  };

  return (
    <div ref={containerRef} className="relative w-full sm:w-80">
      <form onSubmit={submit} role="search">
        <input
          type="search"
          value={value}
          onChange={(e) => setValue(e.target.value)}
          onFocus={() => suggestions.length > 0 && setOpen(true)}
          placeholder="Search tents, jackets, watches…"
          aria-label="Search products"
          className={cn(
            "w-full rounded-md border border-input bg-background px-3 py-2 text-sm",
            "placeholder:text-muted-foreground",
            "focus:outline-none focus:ring-2 focus:ring-brand/40",
          )}
        />
      </form>
      {open && suggestions.length > 0 ? (
        <ul
          role="listbox"
          className={cn(
            "absolute z-20 mt-1 w-full overflow-hidden rounded-md border border-border bg-card text-card-foreground shadow-lg",
          )}
        >
          {suggestions.map((s) => (
            <li key={s.id}>
              <Link
                href={`/${s.slug}`}
                onClick={() => setOpen(false)}
                className="flex items-center justify-between gap-3 px-3 py-2 text-sm hover:bg-muted"
              >
                <span className="truncate">
                  <span className="text-muted-foreground text-xs uppercase tracking-wide mr-2">
                    {s.brand}
                  </span>
                  {s.name}
                </span>
                <span className="text-xs text-muted-foreground capitalize">{s.category}</span>
              </Link>
            </li>
          ))}
        </ul>
      ) : null}
      {pending ? (
        <span className="pointer-events-none absolute right-3 top-2.5 text-xs text-muted-foreground">
          …
        </span>
      ) : null}
    </div>
  );
}
