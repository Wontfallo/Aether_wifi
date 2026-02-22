/**
 * Aether — Environment Doctor
 *
 * Detects the runtime environment (native Linux vs WSL2), checks for
 * USB adapter presence, Monitor mode capability, and provides actionable
 * remediation commands for common issues.
 */

import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
    Stethoscope, CheckCircle2, AlertTriangle, XCircle,
    Copy, RefreshCw
} from "lucide-react";
import type { NetworkInterface } from "../types/capture";

type CheckStatus = "pass" | "warn" | "fail" | "loading";

interface DiagnosticCheck {
    id: string;
    label: string;
    status: CheckStatus;
    detail: string;
    remediation?: string;
}

export function EnvironmentDoctor() {
    const [checks, setChecks] = useState<DiagnosticCheck[]>([]);
    const [isRunning, setIsRunning] = useState(false);
    const [copiedId, setCopiedId] = useState<string | null>(null);

    const runDiagnostics = useCallback(async () => {
        setIsRunning(true);
        const results: DiagnosticCheck[] = [];

        // Check 1: Can we enumerate interfaces?
        try {
            const interfaces = await invoke<NetworkInterface[]>("list_interfaces");

            results.push({
                id: "interfaces",
                label: "Network Interface Discovery",
                status: interfaces.length > 0 ? "pass" : "warn",
                detail: interfaces.length > 0
                    ? `Found ${interfaces.length} interface(s): ${interfaces.map(i => i.name).join(", ")}`
                    : "No network interfaces detected.",
            });

            // Check 2: Do we have wireless interfaces?
            const wireless = interfaces.filter(i => i.is_wireless);
            results.push({
                id: "wireless",
                label: "Wireless Adapter Detection",
                status: wireless.length > 0 ? "pass" : "fail",
                detail: wireless.length > 0
                    ? `Found ${wireless.length} wireless adapter(s): ${wireless.map(w => `${w.name} [${w.driver || "unknown driver"}]`).join(", ")}`
                    : "No wireless adapters found.",
                remediation: wireless.length === 0
                    ? "If using WSL2, run this PowerShell command on the Windows host to attach your USB WiFi adapter:\n\n.\\scripts\\attach_wsl_wifi.ps1 -WslHost \"kali-linux\" -AdapterName \"802.11\"\n\nOr manually:\nusbipd list\nusbipd bind --busid <BUSID>\nusbipd attach --wsl --busid <BUSID>"
                    : undefined,
            });

            // Check 3: Is any interface in Monitor mode?
            const monitorIfaces = wireless.filter(w => w.mode === "monitor");
            results.push({
                id: "monitor",
                label: "Monitor Mode Capability",
                status: monitorIfaces.length > 0 ? "pass" : "warn",
                detail: monitorIfaces.length > 0
                    ? `${monitorIfaces.map(i => i.name).join(", ")} in Monitor mode.`
                    : "No interfaces in Monitor mode. Required for packet capture.",
                remediation: monitorIfaces.length === 0 && wireless.length > 0
                    ? `Set monitor mode with:\nsudo ip link set ${wireless[0].name} down\nsudo iw ${wireless[0].name} set type monitor\nsudo ip link set ${wireless[0].name} up\n\nOr use the toggle in Dashboard / Settings.`
                    : undefined,
            });

            // Check 4: Detect WSL environment
            // We can infer WSL from interface names or by checking for known WSL patterns
            const hasEth0 = interfaces.some(i => i.name === "eth0");
            const noWlan = wireless.length === 0;
            const probableWSL = hasEth0 && noWlan;

            // probableWSL is used below to conditionally add a WSL diagnostic check

            if (probableWSL) {
                results.push({
                    id: "wsl",
                    label: "WSL2 Environment Detected",
                    status: "warn",
                    detail: "Running under WSL2. USB WiFi adapters require usbipd passthrough from Windows.",
                    remediation: "Run this PowerShell command on the Windows host:\n\n.\\scripts\\attach_wsl_wifi.ps1\n\nOr install usbipd:\nwinget install --exact dorssel.usbipd-win\n\nThen:\nusbipd list\nusbipd bind --busid <BUSID>\nusbipd attach --wsl --busid <BUSID>",
                });
            }

            // Check 5: Driver recommendations for common chipsets
            for (const iface of wireless) {
                if (iface.driver) {
                    const driver = iface.driver.toLowerCase();
                    if (driver.includes("rtl88") || driver.includes("8812") || driver.includes("8814")) {
                        results.push({
                            id: `driver-${iface.name}`,
                            label: `Driver Check: ${iface.name}`,
                            status: "pass",
                            detail: `Realtek chipset detected (${iface.driver}). DKMS driver recommended for monitor mode.`,
                            remediation: `For best monitor mode support, install the DKMS driver:\n\nsudo apt install -y dkms\ngit clone https://github.com/aircrack-ng/rtl8812au\ncd rtl8812au\nsudo make dkms_install`,
                        });
                    } else if (driver.includes("ath9k") || driver.includes("ath10k")) {
                        results.push({
                            id: `driver-${iface.name}`,
                            label: `Driver Check: ${iface.name}`,
                            status: "pass",
                            detail: `Atheros chipset (${iface.driver}) — excellent monitor mode support natively.`,
                        });
                    }
                }
            }
        } catch (err: unknown) {
            const msg = typeof err === "string" ? err : JSON.stringify(err);
            results.push({
                id: "interfaces",
                label: "Network Interface Discovery",
                status: "fail",
                detail: `Failed to enumerate interfaces: ${msg}`,
                remediation: "Ensure Aether-Core is running with appropriate privileges (sudo on Linux).",
            });
        }

        setChecks(results);
        setIsRunning(false);
    }, []);

    // Run diagnostics on mount
    useEffect(() => {
        runDiagnostics();
    }, [runDiagnostics]);

    const copyToClipboard = (text: string, id: string) => {
        navigator.clipboard.writeText(text).then(() => {
            setCopiedId(id);
            setTimeout(() => setCopiedId(null), 2000);
        });
    };

    const StatusIcon = ({ status }: { status: CheckStatus }) => {
        switch (status) {
            case "pass": return <CheckCircle2 className="w-4 h-4 text-radar-green" />;
            case "warn": return <AlertTriangle className="w-4 h-4 text-radar-yellow" />;
            case "fail": return <XCircle className="w-4 h-4 text-destructive" />;
            case "loading": return <RefreshCw className="w-4 h-4 text-primary animate-spin" />;
        }
    };

    const passCount = checks.filter(c => c.status === "pass").length;
    const warnCount = checks.filter(c => c.status === "warn").length;
    const failCount = checks.filter(c => c.status === "fail").length;

    return (
        <div className="h-full flex flex-col animate-in fade-in slide-in-from-bottom-4 duration-500">
            <header className="mb-6 flex justify-between items-start">
                <div>
                    <h1 className="text-3xl font-mono font-bold tracking-tight text-foreground flex items-center gap-3">
                        <Stethoscope className="w-8 h-8 text-radar-green" />
                        <span className="text-glow" style={{ textShadow: "0 0 10px rgba(0,255,102,0.4)" }}>DOCTOR</span>{" "}
                        <span className="opacity-50 text-muted-foreground font-sans text-2xl font-normal">// ENVIRONMENT</span>
                    </h1>
                    <p className="text-muted-foreground mt-2 font-mono text-sm uppercase tracking-wider">
                        System Health & Configuration Diagnostics
                    </p>
                </div>
                <button
                    onClick={runDiagnostics}
                    disabled={isRunning}
                    className="flex items-center gap-2 px-4 py-2 rounded font-mono text-sm uppercase tracking-widest border border-radar-green/50 text-radar-green bg-radar-green/10 hover:bg-radar-green hover:text-black transition-all disabled:opacity-30"
                >
                    <RefreshCw className={`w-4 h-4 ${isRunning ? "animate-spin" : ""}`} />
                    Re-scan
                </button>
            </header>

            {/* Summary Stats */}
            <div className="grid grid-cols-3 gap-4 mb-6">
                <div className="glass-panel p-4 rounded-xl text-center">
                    <p className="text-radar-green font-mono text-3xl font-bold">{passCount}</p>
                    <p className="text-muted-foreground font-mono text-[10px] uppercase tracking-widest mt-1">Passed</p>
                </div>
                <div className="glass-panel p-4 rounded-xl text-center">
                    <p className="text-radar-yellow font-mono text-3xl font-bold">{warnCount}</p>
                    <p className="text-muted-foreground font-mono text-[10px] uppercase tracking-widest mt-1">Warnings</p>
                </div>
                <div className="glass-panel p-4 rounded-xl text-center">
                    <p className="text-destructive font-mono text-3xl font-bold">{failCount}</p>
                    <p className="text-muted-foreground font-mono text-[10px] uppercase tracking-widest mt-1">Failed</p>
                </div>
            </div>

            {/* Diagnostics List */}
            <div className="flex-1 overflow-auto space-y-3">
                {checks.map((check) => (
                    <div
                        key={check.id}
                        className={`glass-panel rounded-xl border p-4 transition-all ${check.status === "fail" ? "border-destructive/40" :
                            check.status === "warn" ? "border-radar-yellow/30" :
                                "border-border/40"
                            }`}
                    >
                        <div className="flex items-center gap-3 mb-1">
                            <StatusIcon status={check.status} />
                            <span className="font-mono text-sm font-bold text-foreground">{check.label}</span>
                        </div>
                        <p className="font-mono text-xs text-muted-foreground ml-7">{check.detail}</p>

                        {check.remediation && (
                            <div className="mt-3 ml-7 bg-black/40 border border-border/30 rounded-lg p-3 relative group">
                                <button
                                    onClick={() => copyToClipboard(check.remediation!, check.id)}
                                    className="absolute top-2 right-2 opacity-0 group-hover:opacity-100 transition-opacity text-muted-foreground hover:text-foreground"
                                    title="Copy to clipboard"
                                >
                                    <Copy className="w-3.5 h-3.5" />
                                </button>
                                {copiedId === check.id && (
                                    <span className="absolute top-2 right-8 font-mono text-[10px] text-radar-green">Copied!</span>
                                )}
                                <pre className="font-mono text-[11px] text-primary/80 whitespace-pre-wrap leading-relaxed">{check.remediation}</pre>
                            </div>
                        )}
                    </div>
                ))}

                {isRunning && checks.length === 0 && (
                    <div className="flex flex-col items-center justify-center py-16">
                        <RefreshCw className="w-8 h-8 text-primary animate-spin mb-4" />
                        <p className="font-mono text-sm text-muted-foreground uppercase tracking-widest">Scanning environment...</p>
                    </div>
                )}
            </div>
        </div>
    );
}
