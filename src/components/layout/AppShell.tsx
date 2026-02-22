import { ReactNode } from "react";
import { Sidebar } from "./Sidebar";

interface AppShellProps {
    children: ReactNode;
}

export function AppShell({ children }: AppShellProps) {
    return (
        <div className="flex h-screen w-full bg-background text-foreground overflow-hidden">
            <Sidebar />

            <div className="flex-1 flex flex-col relative w-full h-full min-w-0">
                {/* Subtle background glow effect for depth */}
                <div className="absolute top-[-10%] left-[-10%] w-[40%] h-[40%] bg-primary/5 blur-[120px] rounded-full pointer-events-none"></div>

                {/* Main Content Area */}
                <main className="flex-1 relative overflow-auto p-4 md:p-6 lg:p-8 z-10 no-scrollbar">
                    {children}
                </main>
            </div>
        </div>
    );
}
