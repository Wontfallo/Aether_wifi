/**
 * Aether — Shared UI Components (shadcn wrappers)
 *
 * Thin wrappers around shadcn/ui components that provide the API
 * expected by Aether page components. All styling comes from shadcn.
 */
import { type ReactNode } from "react";
import { cn } from "@/lib/utils";
import { Card, CardContent } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs";
import {
  Table, TableBody, TableCell, TableHead, TableHeader, TableRow,
} from "@/components/ui/table";
import { Loader2 } from "lucide-react";

/* ─── Glass Card ─── */
export function GlassCard({
  children, className,
}: {
  children: ReactNode; className?: string; accent?: string;
}) {
  return (
    <Card className={cn("bg-card/80 backdrop-blur-xl", className)}>
      <CardContent className="p-6">{children}</CardContent>
    </Card>
  );
}

/* ─── Stat Card ─── */
export function StatCard({
  label, value, accent = "text-primary",
}: {
  label: string; value: string | number; accent?: string;
}) {
  return (
    <Card className="bg-card/80 backdrop-blur-xl">
      <CardContent className="p-6">
        <p className="text-muted-foreground font-mono text-xs uppercase tracking-widest mb-2">{label}</p>
        <h2 className={cn("text-3xl font-bold font-mono", accent)}>{value}</h2>
      </CardContent>
    </Card>
  );
}

/* ─── Status Badge ─── */
const badgeVariantMap = {
  active: "default" as const,
  success: "default" as const,
  inactive: "secondary" as const,
  error: "destructive" as const,
  warning: "outline" as const,
};

export function StatusBadge({
  status, label, pulse = true,
}: {
  status: "active" | "inactive" | "error" | "warning" | "success";
  label?: string; pulse?: boolean;
}) {
  const defaultLabels = { active: "Active", inactive: "Inactive", error: "Error", warning: "Warning", success: "Success" };
  const dotColors = { active: "bg-green-400", success: "bg-green-400", inactive: "bg-muted-foreground", error: "bg-destructive", warning: "bg-yellow-400" };

  return (
    <Badge variant={badgeVariantMap[status]} className="gap-1.5 font-mono text-[10px] uppercase tracking-wider">
      <span className={cn("inline-block w-1.5 h-1.5 rounded-full", dotColors[status], pulse && status === "active" && "animate-pulse")} />
      {label || defaultLabels[status]}
    </Badge>
  );
}

/* ─── Action Button ─── */
export function ActionButton({
  children, onClick, variant = "primary", size = "md", disabled, loading, className,
}: {
  children: ReactNode; onClick?: () => void;
  variant?: "primary" | "destructive" | "ghost";
  size?: "sm" | "md" | "lg";
  disabled?: boolean; loading?: boolean; className?: string;
}) {
  const variantMap = { primary: "default" as const, destructive: "destructive" as const, ghost: "ghost" as const };
  const sizeMap = { sm: "sm" as const, md: "default" as const, lg: "lg" as const };

  return (
    <Button
      onClick={onClick}
      disabled={disabled || loading}
      variant={variantMap[variant]}
      size={sizeMap[size]}
      className={cn("font-mono uppercase tracking-widest", className)}
    >
      {loading && <Loader2 className="w-4 h-4 animate-spin" />}
      {children}
    </Button>
  );
}

/* ─── Page Header ─── */
export function PageHeader({
  icon, title, subtitle, description, children,
}: {
  icon: ReactNode; title: string; subtitle: string;
  description?: string; accent?: string; children?: ReactNode;
}) {
  return (
    <header className="mb-6 flex justify-between items-start">
      <div>
        <h1 className="text-2xl font-mono font-bold tracking-tight text-foreground flex items-center gap-3">
          {icon}
          <span className="text-glow">{title}</span>
          <span className="text-muted-foreground font-normal text-lg">// {subtitle}</span>
        </h1>
        {description && (
          <p className="text-muted-foreground mt-1 font-mono text-xs uppercase tracking-wider">{description}</p>
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

export function DataTable<T extends Record<string, unknown>>({
  columns, data, keyField, emptyMessage = "No data.", onRowClick, maxHeight,
}: {
  columns: Column<T>[]; data: T[]; keyField: string;
  emptyMessage?: string; onRowClick?: (item: T) => void; maxHeight?: string;
}) {
  return (
    <div className="rounded-xl border overflow-hidden" style={maxHeight ? { maxHeight, overflowY: "auto" } : undefined}>
      <Table>
        <TableHeader>
          <TableRow>
            {columns.map((col) => (
              <TableHead
                key={col.key}
                className={cn(
                  "font-mono text-[11px] uppercase tracking-widest",
                  col.align === "center" && "text-center",
                  col.align === "right" && "text-right",
                )}
              >
                {col.label}
              </TableHead>
            ))}
          </TableRow>
        </TableHeader>
        <TableBody>
          {data.length === 0 ? (
            <TableRow>
              <TableCell colSpan={columns.length} className="h-24 text-center text-muted-foreground italic">
                {emptyMessage}
              </TableCell>
            </TableRow>
          ) : (
            data.map((item) => (
              <TableRow
                key={String(item[keyField])}
                onClick={() => onRowClick?.(item)}
                className={onRowClick ? "cursor-pointer" : undefined}
              >
                {columns.map((col) => (
                  <TableCell
                    key={col.key}
                    className={cn(
                      "font-mono text-sm",
                      col.align === "center" && "text-center",
                      col.align === "right" && "text-right",
                    )}
                  >
                    {col.render ? col.render(item) : String(item[col.key] ?? "")}
                  </TableCell>
                ))}
              </TableRow>
            ))
          )}
        </TableBody>
      </Table>
    </div>
  );
}

/* ─── Tab Bar ─── */
export function TabBar({
  tabs, active, onChange,
}: {
  tabs: { id: string; label: string; icon?: ReactNode }[];
  active: string;
  onChange: (id: string) => void;
}) {
  return (
    <Tabs value={active} onValueChange={onChange}>
      <TabsList>
        {tabs.map((tab) => (
          <TabsTrigger key={tab.id} value={tab.id} className="gap-1.5 font-mono text-xs uppercase tracking-widest">
            {tab.icon}
            {tab.label}
          </TabsTrigger>
        ))}
      </TabsList>
    </Tabs>
  );
}

/* ─── Input Field ─── */
export function InputField({
  label, value, onChange, placeholder, type = "text", className,
}: {
  label: string; value: string; onChange: (value: string) => void;
  placeholder?: string; type?: string; className?: string; mono?: boolean;
}) {
  return (
    <div className={className}>
      <Label className="font-mono text-xs uppercase tracking-widest mb-2 block">{label}</Label>
      <Input
        type={type}
        placeholder={placeholder}
        value={value}
        onChange={(e) => onChange(e.target.value)}
        className="font-mono"
      />
    </div>
  );
}

/* ─── Select Field ─── */
export function SelectField({
  label, value, onChange, options, className,
}: {
  label: string; value: string; onChange: (value: string) => void;
  options: { value: string; label: string }[]; className?: string;
}) {
  return (
    <div className={className}>
      <Label className="font-mono text-xs uppercase tracking-widest mb-2 block">{label}</Label>
      <select
        value={value}
        onChange={(e) => onChange(e.target.value)}
        className="flex h-8 w-full rounded-lg border border-input bg-transparent px-2.5 py-1 font-mono text-sm transition-colors focus-visible:border-ring focus-visible:ring-3 focus-visible:ring-ring/50"
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
      <div className="w-16 h-1.5 bg-muted rounded-full overflow-hidden">
        <div className={cn("h-full rounded-full", color)} style={{ width: `${pct}%` }} />
      </div>
      <span className={cn("font-bold font-mono text-xs", textColor)}>{rssi}</span>
    </div>
  );
}

/* ─── Empty State ─── */
export function EmptyState({
  icon, title, description,
}: {
  icon: ReactNode; title: string; description?: string;
}) {
  return (
    <div className="flex flex-col items-center justify-center py-16 text-muted-foreground">
      <div className="mb-4 opacity-30">{icon}</div>
      <p className="font-mono text-sm uppercase tracking-widest mb-1">{title}</p>
      {description && <p className="text-xs opacity-60">{description}</p>}
    </div>
  );
}

// Re-export shadcn primitives for direct use
export { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
export { Badge } from "@/components/ui/badge";
export { Button } from "@/components/ui/button";
export { Input } from "@/components/ui/input";
export { Label } from "@/components/ui/label";
export { Tabs, TabsList, TabsTrigger, TabsContent } from "@/components/ui/tabs";
export {
  Table, TableBody, TableCell, TableHead, TableHeader, TableRow,
} from "@/components/ui/table";
export { Separator } from "@/components/ui/separator";
export { Switch } from "@/components/ui/switch";
export { Textarea } from "@/components/ui/textarea";
export {
  Dialog, DialogContent, DialogDescription, DialogHeader, DialogTitle, DialogTrigger,
} from "@/components/ui/dialog";
export {
  Select, SelectContent, SelectItem, SelectTrigger, SelectValue,
} from "@/components/ui/select";
export { ScrollArea } from "@/components/ui/scroll-area";
