import type { Metadata } from "next";
import Link from "next/link";
import "./globals.css";

export const metadata: Metadata = {
  title: {
    default: "GearNest — outdoor & fitness gear, compared",
    template: "%s · GearNest",
  },
  description:
    "Semantic search and price comparison across 8 outdoor retailers. Ask AI about any product, grounded in real specs and community reviews.",
};

export default function RootLayout({ children }: { children: React.ReactNode }) {
  return (
    <html lang="en" suppressHydrationWarning>
      <body className="min-h-screen bg-background text-foreground antialiased">
        <header className="border-b border-border bg-background/80 backdrop-blur sticky top-0 z-30">
          <div className="mx-auto flex max-w-7xl items-center justify-between gap-6 px-4 py-3 sm:px-6">
            <Link href="/" className="flex items-center gap-2 font-semibold tracking-tight">
              <span className="inline-block h-6 w-6 rounded-md bg-brand" aria-hidden />
              <span>GearNest</span>
            </Link>
            <nav className="flex items-center gap-4 text-sm text-muted-foreground">
              <Link href="/" className="hover:text-foreground">Catalog</Link>
              <a
                href="https://github.com/anthropics/claude-code"
                className="hover:text-foreground"
                target="_blank"
                rel="noreferrer"
              >
                Spec
              </a>
            </nav>
          </div>
        </header>
        <main className="mx-auto max-w-7xl px-4 py-6 sm:px-6">{children}</main>
        <footer className="border-t border-border mt-16 py-8 text-center text-xs text-muted-foreground">
          Catalog data is illustrative · GearNest Phase 1 preview
        </footer>
      </body>
    </html>
  );
}
