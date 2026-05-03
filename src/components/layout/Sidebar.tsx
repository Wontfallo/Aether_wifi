import {
  Activity, Target, Zap, LayoutDashboard, Terminal, Stethoscope,
  Settings2, Radar, Eye, Wrench,
} from "lucide-react";
import { Link, useLocation } from "react-router-dom";
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from "../ui/tooltip";
import { cn } from "@/lib/utils";

const NAV_ITEMS = [
    { id: "dashboard", icon: LayoutDashboard, label: "Dashboard", path: "/" },
    { id: "spectrum", icon: Activity, label: "Spectrum", path: "/spectrum" },
    { id: "hunt", icon: Target, label: "Hunt Mode", path: "/hunt" },
    { id: "strike", icon: Zap, label: "Strike", path: "/strike" },
    { id: "recon", icon: Radar, label: "Recon", path: "/recon" },
    { id: "sniffer", icon: Eye, label: "Sniffer", path: "/sniffer" },
    { id: "tools", icon: Wrench, label: "Tools", path: "/tools" },
    { id: "doctor", icon: Stethoscope, label: "Env Doctor", path: "/doctor" },
    { id: "settings", icon: Settings2, label: "Settings", path: "/settings" },
];

export function Sidebar() {
    const location = useLocation();

    return (
        <aside className="w-14 h-screen flex flex-col items-center py-4 border-r bg-card/50 backdrop-blur-xl shrink-0 z-50 overflow-hidden">
            <div className="mb-6 text-primary">
                <Terminal className="w-6 h-6" strokeWidth={1.5} />
            </div>

            <nav className="flex-1 flex flex-col gap-1 w-full px-1.5">
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
                                            "w-11 h-11 flex items-center justify-center rounded-lg transition-all relative",
                                            isActive
                                                ? "bg-primary/15 text-primary"
                                                : "text-muted-foreground hover:bg-muted hover:text-foreground"
                                        )}
                                    >
                                        {isActive && (
                                            <div className="absolute left-0 top-1/2 -translate-y-1/2 w-[3px] h-5 bg-primary rounded-r-full" />
                                        )}
                                        <Icon className="w-4.5 h-4.5" strokeWidth={isActive ? 2 : 1.5} />
                                    </Link>
                                </TooltipTrigger>
                                <TooltipContent side="right" sideOffset={8} className="font-mono text-[10px] uppercase tracking-wider">
                                    {item.label}
                                </TooltipContent>
                            </Tooltip>
                        );
                    })}
                </TooltipProvider>
            </nav>

            <div className="mt-auto px-2">
                <div className="w-8 h-8 rounded-full bg-secondary flex items-center justify-center">
                    <div className="w-2 h-2 rounded-full bg-radar-green animate-pulse" />
                </div>
            </div>
        </aside>
    );
}
