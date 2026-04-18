import { type ReactNode } from "react";
import { cn } from "./tooltip";

/* ─── Glass Card ─── */
interface GlassCardProps {
  children: ReactNode;
  className?: string;
  accent?: "primary" | "destructive" | "green" | "yellow" | "none";
}

const accentColors = {
  primary: "group-hover:bg-primary",
  destructive: "group-hover:bg-destructive",
  green: "group-hover:bg-radar-green",
  yellow: "group-hover:bg-radar-yellow",
  none: "",
};

export function GlassCard({ children, className, accent = "primary" }: GlassCardProps) {
  return (
    <div className={cn("glass-panel rounded-xl relative overflow-hidden group", className)}>
      {accent !== "none" && (
        <div className={cn("absolute top-0 left-0 w-1 h-full bg-border transition-colors", accentColors[accent])} />
      )}
      {children}
    </div>
  );
}

/* ─── Stat Card ─── */
interface StatCardProps {
  label: string;
  value: string | number;
  accent?: "primary" | "destructive" | "green" | "yellow";
}

const statColors = {
  primary: "text-primary",
  destructive: "text-destructive",
  green: "text-radar-green",
  yellow: "text-radar-yellow",
};

export function StatCard({ label, value, accent = "primary" }: StatCardProps) {
  return (
    <GlassCard accent={accent} className="p-6">
      <p className="text-muted-foreground font-mono text-xs uppercase tracking-widest mb-2">{label}</p>
      <h2 className={cn("text-4xl font-bold font-mono", statColors[accent])}>{value}</h2>
    </GlassCard>
  );
}

/* ─── Status Badge ─── */
interface StatusBadgeProps {
  status: "active" | "inactive" | "error" | "warning" | "success";
  label?: string;
  pulse?: boolean;
}

const badgeConfig = {
  active: { dot: "bg-radar-green", text: "text-radar-green", label: "Active" },
  inactive: { dot: "bg-muted-foreground", text: "text-muted-foreground", label: "Inactive" },
  error: { dot: "bg-destructive", text: "text-destructive", label: "Error" },
  warning: { dot: "bg-radar-yellow", text: "text-radar-yellow", label: "Warning" },
  success: { dot: "bg-radar-green", text: "text-radar-green", label: "Success" },
};

export function StatusBadge({ status, label, pulse = true }: StatusBadgeProps) {
  const config = badgeConfig[status];
  return (
    <div className="flex gap-2 items-center">
      <div className={cn("w-2 h-2 rounded-full", config.dot, pulse && status === "active" && "animate-pulse-fast")} />
      <span className={cn("font-mono text-[10px] uppercase tracking-wider", config.text)}>
        {label || config.label}
      </span>
    </div>
  );
}

/* ─── Action Button ─── */
interface ActionButtonProps {
  children: ReactNode;
  onClick?: () => void;
  variant?: "primary" | "destructive" | "ghost";
  size?: "sm" | "md" | "lg";
  disabled?: boolean;
  loading?: boolean;
  className?: string;
}

export function ActionButton({
  children, onClick, variant = "primary", size = "md", disabled, loading, className,
}: ActionButtonProps) {
  const variants = {
    primary: "bg-primary/10 border-primary text-primary hover:bg-primary hover:text-black",
    destructive: "bg-destructive/10 border-destructive text-destructive hover:bg-destructive hover:text-white",
    ghost: "bg-transparent border-border text-muted-foreground hover:bg-muted/50 hover:text-foreground",
  };
  const sizes = {
    sm: "px-3 py-1.5 text-[11px]",
    md: "px-4 py-2 text-sm",
    lg: "px-6 py-3 text-sm",
  };

  return (
    <button
      onClick={onClick}
      disabled={disabled || loading}
      className={cn(
        "flex items-center gap-2 rounded font-mono uppercase tracking-widest border transition-all disabled:opacity-50 disabled:cursor-not-allowed",
        variants[variant],
        sizes[size],
        className,
      )}
    >
      {loading && <div className="w-4 h-4 border-2 border-current border-t-transparent rounded-full animate-spin" />}
      {children}
    </button>
  );
}

/* ─── Page Header ─── */
interface PageHeaderProps {
  icon: ReactNode;
  title: string;
  subtitle: string;
  description?: string;
  accent?: string;
  children?: ReactNode;
}

export function PageHeader({ icon, title, subtitle, description, accent = "text-primary", children }: PageHeaderProps) {
  return (
    <header className="mb-8 flex justify-between items-start">
      <div>
        <h1 className="text-3xl font-mono font-bold tracking-tight text-foreground flex items-center gap-3">
          {icon}
          <span className={cn("text-glow", accent)}>{title}</span>
          <span className="opacity-50 text-muted-foreground font-sans text-2xl font-normal">// {subtitle}</span>
        </h1>
        {description && (
          <p className="text-muted-foreground mt-2 font-mono text-sm uppercase tracking-wider">{description}</p>
        )}
      </div>
      {children && <div className="flex items-center gap-3">{children}</div>}
    </header>
  );
}

/* ─── Data Table ─── */
interface Column<T> {
  key: string;
  label: string;
  align?: "left" | "center" | "right";
  render?: (item: T) => ReactNode;
}

interface DataTableProps<T> {
  columns: Column<T>[];
  data: T[];
  keyField: string;
  emptyMessage?: string;
  maxHeight?: string;
  onRowClick?: (item: T) => void;
}

export function DataTable<T extends Record<string, unknown>>({
  columns, data, keyField, emptyMessage = "No data.", maxHeight, onRowClick,
}: DataTableProps<T>) {
  return (
    <div className="glass-panel rounded-xl overflow-hidden border border-border/40 flex flex-col" style={maxHeight ? { maxHeight } : undefined}>
      <div className="overflow-auto flex-1 bg-black/20">
        <table className="w-full text-left font-mono text-sm border-collapse">
          <thead className="sticky top-0 bg-background/95 backdrop-blur z-10 border-b border-border/40">
            <tr>
              {columns.map((col) => (
                <th
                  key={col.key}
                  className={cn(
                    "py-3 px-5 font-normal text-muted-foreground uppercase tracking-widest text-[11px]",
                    col.align === "center" && "text-center",
                    col.align === "right" && "text-right",
                  )}
                >
                  {col.label}
                </th>
              ))}
            </tr>
          </thead>
          <tbody>
            {data.length === 0 ? (
              <tr>
                <td colSpan={columns.length} className="py-8 text-center text-muted-foreground italic text-xs">
                  {emptyMessage}
                </td>
              </tr>
            ) : (
              data.map((item) => (
                <tr
                  key={String(item[keyField])}
                  onClick={() => onRowClick?.(item)}
                  className={cn(
                    "border-b border-border/10 hover:bg-white/5 transition-colors",
                    onRowClick && "cursor-pointer",
                  )}
                >
                  {columns.map((col) => (
                    <td
                      key={col.key}
                      className={cn(
                        "py-2.5 px-5 text-foreground/80",
                        col.align === "center" && "text-center",
                        col.align === "right" && "text-right",
                      )}
                    >
                      {col.render ? col.render(item) : String(item[col.key] ?? "")}
                    </td>
                  ))}
                </tr>
              ))
            )}
          </tbody>
        </table>
      </div>
    </div>
  );
}

/* ─── Tab Bar ─── */
interface TabBarProps {
  tabs: { id: string; label: string; icon?: ReactNode }[];
  active: string;
  onChange: (id: string) => void;
}

export function TabBar({ tabs, active, onChange }: TabBarProps) {
  return (
    <div className="flex gap-1 p-1 bg-muted/30 rounded-lg border border-border/40 mb-6">
      {tabs.map((tab) => (
        <button
          key={tab.id}
          onClick={() => onChange(tab.id)}
          className={cn(
            "flex items-center gap-2 px-4 py-2 rounded-md font-mono text-xs uppercase tracking-widest transition-all",
            active === tab.id
              ? "bg-primary/15 text-primary shadow-sm border border-primary/20"
              : "text-muted-foreground hover:text-foreground hover:bg-muted/50",
          )}
        >
          {tab.icon}
          {tab.label}
        </button>
      ))}
    </div>
  );
}

/* ─── Input Field ─── */
interface InputFieldProps {
  label: string;
  value: string;
  onChange: (value: string) => void;
  placeholder?: string;
  type?: string;
  className?: string;
  mono?: boolean;
}

export function InputField({ label, value, onChange, placeholder, type = "text", className, mono = true }: InputFieldProps) {
  return (
    <div className={className}>
      <label className="text-xs font-mono uppercase tracking-widest text-muted-foreground mb-2 block">{label}</label>
      <input
        type={type}
        placeholder={placeholder}
        value={value}
        onChange={(e) => onChange(e.target.value)}
        className={cn(
          "w-full bg-black/40 border border-border/60 rounded px-4 py-2 text-sm text-foreground focus:border-primary focus:outline-none transition-colors",
          mono && "font-mono",
        )}
      />
    </div>
  );
}

/* ─── Select Field ─── */
interface SelectFieldProps {
  label: string;
  value: string;
  onChange: (value: string) => void;
  options: { value: string; label: string }[];
  className?: string;
}

export function SelectField({ label, value, onChange, options, className }: SelectFieldProps) {
  return (
    <div className={className}>
      <label className="text-xs font-mono uppercase tracking-widest text-muted-foreground mb-2 block">{label}</label>
      <select
        value={value}
        onChange={(e) => onChange(e.target.value)}
        className="w-full bg-black/40 border border-border/60 rounded px-4 py-2 font-mono text-sm text-foreground focus:border-primary focus:outline-none transition-colors appearance-none"
      >
        {options.map((opt) => (
          <option key={opt.value} value={opt.value}>{opt.label}</option>
        ))}
      </select>
    </div>
  );
}

/* ─── Signal Bar ─── */
export function SignalBar({ rssi }: { rssi: number }) {
  const color = rssi > -50 ? "bg-radar-green" : rssi > -70 ? "bg-primary" : rssi > -85 ? "bg-radar-yellow" : "bg-destructive";
  const textColor = rssi > -50 ? "text-radar-green" : rssi > -70 ? "text-primary" : rssi > -85 ? "text-radar-yellow" : "text-destructive";
  const pct = Math.max(0, Math.min(100, (rssi + 100) * 1.25));

  return (
    <div className="flex items-center gap-3">
      <div className="w-16 h-1.5 bg-black rounded-full overflow-hidden">
        <div className={cn("h-full rounded-full", color)} style={{ width: `${pct}%` }} />
      </div>
      <span className={cn("font-bold font-mono text-xs", textColor)}>{rssi}</span>
    </div>
  );
}

/* ─── Empty State ─── */
export function EmptyState({ icon, title, description }: { icon: ReactNode; title: string; description?: string }) {
  return (
    <div className="flex flex-col items-center justify-center py-16 text-muted-foreground">
      <div className="mb-4 opacity-30">{icon}</div>
      <p className="font-mono text-sm uppercase tracking-widest mb-1">{title}</p>
      {description && <p className="text-xs opacity-60">{description}</p>}
    </div>
  );
}
