import { Activity, Target, Shield, LayoutDashboard, Terminal, Stethoscope, Settings2 } from "lucide-react";
import { Link, useLocation } from "react-router-dom";
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from "../ui/tooltip";
import { cn } from "../ui/tooltip";

const NAV_ITEMS = [
    { id: "dashboard", icon: LayoutDashboard, label: "Dashboard", path: "/" },
    { id: "spectrum", icon: Activity, label: "Spectrum", path: "/spectrum" },
    { id: "hunt", icon: Target, label: "Hunt Mode", path: "/hunt" },
    { id: "audit", icon: Shield, label: "Audit Suite", path: "/audit" },
    { id: "doctor", icon: Stethoscope, label: "Env Doctor", path: "/doctor" },
    { id: "settings", icon: Settings2, label: "Settings", path: "/settings" },
];

export function Sidebar() {
    const location = useLocation();

    return (
        <aside className="w-16 h-screen flex flex-col items-center py-6 glass-panel border-r border-border/50 bg-background/95 shrink-0 z-50 overflow-hidden relative">
            {/* Decorative top accent */}
            <div className="absolute top-0 left-0 w-full h-[2px] bg-primary/40 shadow-glow-primary"></div>

            <div className="mb-10 text-primary">
                <Terminal className="w-7 h-7" strokeWidth={1.5} />
            </div>

            <nav className="flex-1 flex flex-col gap-3 w-full px-2">
                <TooltipProvider delayDuration={0}>
                    {NAV_ITEMS.map((item) => {
                        const isActive = location.pathname === item.path;
                        const Icon = item.icon;

                        return (
                            <Tooltip key={item.id}>
                                <TooltipTrigger asChild>
                                    <Link
                                        to={item.path}
                                        className={cn(
                                            "w-12 h-12 flex flex-col items-center justify-center rounded-lg transition-all duration-300 group relative",
                                            isActive
                                                ? "bg-primary/10 text-primary shadow-[inset_0_0_10px_rgba(0,229,255,0.1)]"
                                                : "text-muted-foreground hover:bg-muted/50 hover:text-foreground"
                                        )}
                                    >
                                        {isActive && (
                                            <div className="absolute left-0 top-1/2 -translate-y-1/2 w-1 h-6 bg-primary rounded-r-full shadow-glow-primary"></div>
                                        )}
                                        <Icon className="w-5 h-5 transition-transform group-hover:scale-110" strokeWidth={isActive ? 2 : 1.5} />
                                    </Link>
                                </TooltipTrigger>
                                <TooltipContent side="right" sideOffset={10} className="font-mono text-[10px] uppercase tracking-wider bg-card text-foreground border-border/60">
                                    {item.label}
                                </TooltipContent>
                            </Tooltip>
                        );
                    })}
                </TooltipProvider>
            </nav>

            <div className="mt-auto px-2">
                <div className="w-10 h-10 rounded-full bg-secondary border border-border/50 flex items-center justify-center">
                    <div className="w-2 h-2 rounded-full bg-radar-green shadow-glow-radar animate-pulse-fast"></div>
                </div>
            </div>
        </aside>
    );
}
