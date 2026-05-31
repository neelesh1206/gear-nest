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
        <div className="border-b border-warning/30 bg-warning/10 text-warning text-xs">
          <p className="mx-auto max-w-7xl px-4 py-1.5 sm:px-6 text-center">
            Demo data · catalog, prices, and chat answers are illustrative for the Phase 1 preview.
          </p>
        </div>
        <header className="border-b border-border bg-background/80 backdrop-blur sticky top-0 z-30">
          <div className="mx-auto flex max-w-7xl items-center justify-between gap-6 px-4 py-3 sm:px-6">
            <Link href="/" className="flex items-center gap-2 font-semibold tracking-tight">
              <Logo className="h-7 w-7" />
              <span>GearNest</span>
            </Link>
            <nav className="flex items-center gap-4 text-sm text-muted-foreground">
              <Link href="/" className="hover:text-foreground">Catalog</Link>
              <a
                href="https://github.com/neelesh1206/gear-nest/blob/main/SPEC.md"
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
          GearNest Phase 1 preview · data refreshed daily from 8 retailers when live
        </footer>
      </body>
    </html>
  );
}

function Logo({ className }: { className?: string }) {
  return (
    <svg
      xmlns="http://www.w3.org/2000/svg"
      viewBox="0 0 32 32"
      role="img"
      aria-label="GearNest"
      className={className}
    >
      <rect width="32" height="32" rx="7" fill="#2f7a3a" />
      <path d="M4 23 L12 9 L17 16.5 L21 12 L28 23 Z" fill="#ffffff" />
      <path d="M10.4 13 L12 9 L13.6 13 L12.6 12.2 L11.4 12.2 Z" fill="#2f7a3a" />
      <path
        d="M5 25 Q16 28.6 27 25"
        fill="none"
        stroke="#ffffff"
        strokeWidth="1.5"
        strokeLinecap="round"
        opacity="0.85"
      />
    </svg>
  );
}
