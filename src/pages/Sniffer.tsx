import { Eye, Play, Square, Radio, ShieldAlert, BarChart3, KeyRound, Ghost, Copy, Download } from "lucide-react";
import ReactECharts from "echarts-for-react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import {
  PageHeader, ActionButton, StatCard, DataTable,
  TabBar, EmptyState, GlassCard, StatusBadge, SignalBar, InputField,
} from "../components/ui/shared";
import type {
  ProbeRequest, DeauthEvent, PacketStats, PmkidCapture, PwnagotchiInfo, RawFrame,
  SaeFrame, MacTrackEntry,
} from "../types/capture";

const MAX_ENTRIES = 500;
const FPS_HISTORY = 60;

function getErrorMessage(error: unknown): string {
  if (typeof error === "string") return error;
  if (error instanceof Error) return error.message;
  try {
    return JSON.stringify(error);
  } catch {
    return "Unknown error";
  }
}

function ErrorBanner({ message, onDismiss }: { message: string; onDismiss: () => void }) {
  return (
    <div className="mb-4 rounded-xl border border-destructive/40 bg-destructive/10 px-4 py-3 font-mono text-sm text-destructive">
      <div className="flex items-start justify-between gap-4">
        <span className="break-all">{message}</span>
        <button onClick={onDismiss} className="opacity-70 transition-opacity hover:opacity-100">×</button>
      </div>
    </div>
  );
}

type SnifferControlProps = {
  interfaceName: string;
  onError: (message: string | null) => void;
};

function formatTime(ms: number): string {
  const d = new Date(ms);
  return d.toLocaleTimeString("en-GB", { hour12: false });
}

function truncate(s: string, len: number): string {
  return s.length > len ? s.slice(0, len) + "…" : s;
}

/* ─────────────────────── Probe Requests Tab ─────────────────────── */

function ProbeRequestsTab({ interfaceName, onError }: SnifferControlProps) {
  const [probes, setProbes] = useState<(ProbeRequest & { _id: number })[]>([]);
  const [running, setRunning] = useState(false);
  const [loading, setLoading] = useState(false);
  const idRef = useRef(0);
  const scrollRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    let unlisten: UnlistenFn | null = null;
    let mounted = true;

    (async () => {
      unlisten = await listen<ProbeRequest>("probe-request", (evt) => {
        if (!mounted) return;
        const id = ++idRef.current;
        setProbes((prev) => {
          const next = [{ ...evt.payload, _id: id }, ...prev];
          return next.length > MAX_ENTRIES ? next.slice(0, MAX_ENTRIES) : next;
        });
      });
    })();

    return () => {
      mounted = false;
      unlisten?.();
    };
  }, []);

  useEffect(() => {
    scrollRef.current?.scrollTo({ top: 0, behavior: "smooth" });
  }, [probes.length]);

  const toggle = async () => {
    setLoading(true);
    onError(null);
    try {
      if (running) {
        await invoke("stop_sniffer");
        setRunning(false);
      } else {
        await invoke("start_sniffer", { interfaceName });
        setRunning(true);
      }
    } catch (error) {
      onError(getErrorMessage(error));
    }
    setLoading(false);
  };

  const uniqueDevices = useMemo(() => new Set(probes.map((p) => p.source_mac)).size, [probes]);
  const uniqueSsids = useMemo(() => new Set(probes.filter((p) => p.ssid).map((p) => p.ssid)).size, [probes]);

  type ProbeRow = ProbeRequest & { _id: number; [key: string]: unknown };
  const columns = useMemo(() => [
    { key: "time", label: "Time", render: (r: ProbeRow) => <span className="text-muted-foreground">{formatTime(r.timestamp_ms)}</span> },
    { key: "source_mac", label: "Source MAC", render: (r: ProbeRow) => <span className="text-primary font-mono">{r.source_mac}</span> },
    { key: "ssid", label: "SSID", render: (r: ProbeRow) => r.ssid ? <span>{r.ssid}</span> : <span className="text-muted-foreground italic">hidden</span> },
    { key: "channel", label: "CH", align: "center" as const },
    { key: "rssi", label: "RSSI", render: (r: ProbeRow) => <SignalBar rssi={r.rssi} /> },
    { key: "vendor", label: "Vendor", render: (r: ProbeRow) => <span className="text-muted-foreground">{r.vendor || "—"}</span> },
  ], []);

  return (
    <div className="flex flex-col gap-6 flex-1 min-h-0">
      <div className="flex items-center gap-4">
        <ActionButton variant={running ? "destructive" : "primary"} onClick={toggle} loading={loading}>
          {running ? <><Square className="w-4 h-4" /> Stop Sniffer</> : <><Play className="w-4 h-4" /> Start Sniffer</>}
        </ActionButton>
        <StatusBadge status={running ? "active" : "inactive"} label={running ? "Capturing" : "Idle"} pulse />
      </div>

      <div className="grid grid-cols-3 gap-4">
        <StatCard label="Total Probes" value={probes.length} accent="primary" />
        <StatCard label="Unique Devices" value={uniqueDevices} accent="green" />
        <StatCard label="Unique SSIDs" value={uniqueSsids} accent="yellow" />
      </div>

      <div ref={scrollRef} className="flex-1 min-h-0">
        {probes.length === 0 ? (
          <EmptyState icon={<Radio className="w-12 h-12" />} title="No Probe Requests" description="Start the sniffer to capture probe requests" />
        ) : (
          <DataTable columns={columns} data={probes as unknown as ProbeRow[]} keyField="_id" maxHeight="calc(100vh - 420px)" />
        )}
      </div>
    </div>
  );
}

/* ─────────────────────── Deauth Monitor Tab ─────────────────────── */

function DeauthMonitorTab({ interfaceName, onError }: SnifferControlProps) {
  const [events, setEvents] = useState<(DeauthEvent & { _id: number })[]>([]);
  const [running, setRunning] = useState(false);
  const [loading, setLoading] = useState(false);
  const [flash, setFlash] = useState(false);
  const idRef = useRef(0);

  useEffect(() => {
    let unlisten: UnlistenFn | null = null;
    let mounted = true;

    (async () => {
      unlisten = await listen<DeauthEvent>("deauth-detected", (evt) => {
        if (!mounted) return;
        const id = ++idRef.current;
        setEvents((prev) => {
          const next = [{ ...evt.payload, _id: id }, ...prev];
          return next.length > MAX_ENTRIES ? next.slice(0, MAX_ENTRIES) : next;
        });
        setFlash(true);
        setTimeout(() => setFlash(false), 1500);
      });
    })();

    return () => {
      mounted = false;
      unlisten?.();
    };
  }, []);

  const toggle = async () => {
    setLoading(true);
    onError(null);
    try {
      if (running) {
        await invoke("stop_sniffer");
        setRunning(false);
      } else {
        await invoke("start_sniffer", { interfaceName });
        setRunning(true);
      }
    } catch (error) {
      onError(getErrorMessage(error));
    }
    setLoading(false);
  };

  type DeauthRow = DeauthEvent & { _id: number; [key: string]: unknown };
  const columns = useMemo(() => [
    { key: "time", label: "Time", render: (r: DeauthRow) => <span className="text-muted-foreground">{formatTime(r.timestamp_ms)}</span> },
    { key: "source_mac", label: "Source", render: (r: DeauthRow) => <span className="text-primary font-mono">{r.source_mac}</span> },
    { key: "dest_mac", label: "Target", render: (r: DeauthRow) => <span className="font-mono">{r.dest_mac}</span> },
    { key: "bssid", label: "BSSID", render: (r: DeauthRow) => <span className="font-mono text-muted-foreground">{r.bssid}</span> },
    { key: "reason_code", label: "Reason", align: "center" as const },
    {
      key: "is_broadcast", label: "Broadcast?", align: "center" as const,
      render: (r: DeauthRow) => r.is_broadcast
        ? <span className="text-destructive font-bold">YES</span>
        : <span className="text-muted-foreground">no</span>,
    },
    { key: "rssi", label: "RSSI", render: (r: DeauthRow) => <SignalBar rssi={r.rssi} /> },
  ], []);

  return (
    <div className="flex flex-col gap-6 flex-1 min-h-0">
      <div className="flex items-center gap-4">
        <ActionButton variant={running ? "destructive" : "primary"} onClick={toggle} loading={loading}>
          {running ? <><Square className="w-4 h-4" /> Stop Monitor</> : <><Play className="w-4 h-4" /> Start Monitor</>}
        </ActionButton>
        <StatusBadge status={running ? "active" : "inactive"} label={running ? "Monitoring" : "Idle"} pulse />
      </div>

      <div className={`transition-all duration-300 rounded-xl ${flash ? "ring-2 ring-destructive shadow-[0_0_30px_rgba(255,23,68,0.3)]" : ""}`}>
        <StatCard label="Deauth Events Detected" value={events.length} accent="destructive" />
      </div>

      {events.length === 0 ? (
        <EmptyState icon={<ShieldAlert className="w-12 h-12" />} title="No Deauth Events" description="Start the monitor to detect deauth/disassociation attacks" />
      ) : (
        <DataTable
          columns={columns}
          data={events as unknown as DeauthRow[]}
          keyField="_id"
          maxHeight="calc(100vh - 380px)"
        />
      )}
    </div>
  );
}

/* ─────────────────────── Packet Stats Tab ─────────────────────── */

function PacketStatsTab({ interfaceName, onError }: SnifferControlProps) {
  const [stats, setStats] = useState<PacketStats | null>(null);
  const [fpsHistory, setFpsHistory] = useState<{ time: string; fps: number }[]>([]);
  const [running, setRunning] = useState(false);
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    let unlisten: UnlistenFn | null = null;
    let mounted = true;

    (async () => {
      unlisten = await listen<PacketStats>("packet-stats", (evt) => {
        if (!mounted) return;
        const s = evt.payload;
        setStats(s);
        setFpsHistory((prev) => {
          const next = [...prev, { time: formatTime(s.timestamp_ms), fps: s.frames_per_second }];
          return next.length > FPS_HISTORY ? next.slice(-FPS_HISTORY) : next;
        });
      });
    })();

    return () => {
      mounted = false;
      unlisten?.();
    };
  }, []);

  const toggle = async () => {
    setLoading(true);
    onError(null);
    try {
      if (running) {
        await invoke("stop_packet_monitor");
        setRunning(false);
      } else {
        await invoke("start_packet_monitor", { interfaceName });
        setRunning(true);
      }
    } catch (error) {
      onError(getErrorMessage(error));
    }
    setLoading(false);
  };

  const donutOption = useMemo(() => {
    if (!stats) return {};
    return {
      backgroundColor: "transparent",
      tooltip: { trigger: "item", formatter: "{b}: {c} ({d}%)" },
      legend: {
        bottom: 0, textStyle: { color: "rgba(255,255,255,0.6)", fontFamily: "JetBrains Mono", fontSize: 10 },
      },
      series: [{
        type: "pie", radius: ["45%", "70%"], center: ["50%", "45%"],
        itemStyle: { borderColor: "hsl(224 71% 4%)", borderWidth: 2, borderRadius: 4 },
        label: { show: false },
        data: [
          { value: stats.management_frames, name: "Management", itemStyle: { color: "#00E5FF" } },
          { value: stats.control_frames, name: "Control", itemStyle: { color: "#FFEA00" } },
          { value: stats.data_frames, name: "Data", itemStyle: { color: "#00FF66" } },
        ],
      }],
    };
  }, [stats]);

  const lineOption = useMemo(() => ({
    backgroundColor: "transparent",
    tooltip: { trigger: "axis" },
    grid: { top: 20, right: 20, bottom: 30, left: 50 },
    xAxis: {
      type: "category",
      data: fpsHistory.map((h) => h.time),
      axisLabel: { color: "rgba(255,255,255,0.4)", fontFamily: "JetBrains Mono", fontSize: 9, rotate: 45 },
      axisLine: { lineStyle: { color: "rgba(255,255,255,0.1)" } },
    },
    yAxis: {
      type: "value", name: "FPS",
      nameTextStyle: { color: "rgba(255,255,255,0.4)", fontFamily: "JetBrains Mono", fontSize: 10 },
      axisLabel: { color: "rgba(255,255,255,0.4)", fontFamily: "JetBrains Mono", fontSize: 10 },
      splitLine: { lineStyle: { color: "rgba(255,255,255,0.05)" } },
    },
    series: [{
      type: "line", smooth: true, showSymbol: false,
      data: fpsHistory.map((h) => h.fps),
      lineStyle: { color: "#00E5FF", width: 2 },
      areaStyle: { color: { type: "linear", x: 0, y: 0, x2: 0, y2: 1, colorStops: [{ offset: 0, color: "rgba(0,229,255,0.25)" }, { offset: 1, color: "rgba(0,229,255,0)" }] } },
    }],
  }), [fpsHistory]);

  return (
    <div className="flex flex-col gap-6 flex-1 min-h-0">
      <div className="flex items-center gap-4">
        <ActionButton variant={running ? "destructive" : "primary"} onClick={toggle} loading={loading}>
          {running ? <><Square className="w-4 h-4" /> Stop Monitor</> : <><Play className="w-4 h-4" /> Start Monitor</>}
        </ActionButton>
        <StatusBadge status={running ? "active" : "inactive"} label={running ? "Monitoring" : "Idle"} pulse />
      </div>

      <div className="grid grid-cols-5 gap-4">
        <StatCard label="Total Frames" value={stats?.total_frames ?? 0} accent="primary" />
        <StatCard label="Management" value={stats?.management_frames ?? 0} accent="primary" />
        <StatCard label="Control" value={stats?.control_frames ?? 0} accent="yellow" />
        <StatCard label="Data" value={stats?.data_frames ?? 0} accent="green" />
        <StatCard label="FPS" value={stats?.frames_per_second ?? 0} accent="primary" />
      </div>

      {!stats ? (
        <EmptyState icon={<BarChart3 className="w-12 h-12" />} title="No Packet Data" description="Start the packet monitor to view frame statistics" />
      ) : (
        <div className="grid grid-cols-2 gap-6 flex-1 min-h-0">
          <GlassCard className="p-4">
            <p className="text-muted-foreground font-mono text-xs uppercase tracking-widest mb-2">Frame Distribution</p>
            <ReactECharts option={donutOption} style={{ height: 280 }} opts={{ renderer: "canvas" }} />
          </GlassCard>
          <GlassCard className="p-4">
            <p className="text-muted-foreground font-mono text-xs uppercase tracking-widest mb-2">Frames Per Second</p>
            <ReactECharts option={lineOption} style={{ height: 280 }} opts={{ renderer: "canvas" }} />
          </GlassCard>
        </div>
      )}
    </div>
  );
}

/* ─────────────────────── PMKID Tab ─────────────────────── */

function PmkidTab({ interfaceName, onError }: SnifferControlProps) {
  const [captures, setCaptures] = useState<(PmkidCapture & { _id: number })[]>([]);
  const [running, setRunning] = useState(false);
  const [loading, setLoading] = useState(false);
  const [copied, setCopied] = useState<number | null>(null);
  const idRef = useRef(0);

  useEffect(() => {
    let unlisten: UnlistenFn | null = null;
    let mounted = true;

    (async () => {
      unlisten = await listen<PmkidCapture>("pmkid-capture", (evt) => {
        if (!mounted) return;
        const id = ++idRef.current;
        setCaptures((prev) => {
          const next = [{ ...evt.payload, _id: id }, ...prev];
          return next.length > MAX_ENTRIES ? next.slice(0, MAX_ENTRIES) : next;
        });
      });
    })();

    return () => {
      mounted = false;
      unlisten?.();
    };
  }, []);

  const toggle = async () => {
    setLoading(true);
    onError(null);
    try {
      if (running) {
        await invoke("stop_pmkid_capture");
        setRunning(false);
      } else {
        await invoke("start_pmkid_capture", { interfaceName });
        setRunning(true);
      }
    } catch (error) {
      onError(getErrorMessage(error));
    }
    setLoading(false);
  };

  type PmkidRow = PmkidCapture & { _id: number; [key: string]: unknown };

  const copyHashcat = useCallback((item: PmkidRow) => {
    navigator.clipboard.writeText(item.hashcat_line).catch(() => {});
    setCopied(item._id);
    setTimeout(() => setCopied(null), 2000);
  }, []);

  const exportAll = useCallback(() => {
    if (captures.length === 0) return;
    const content = captures.map((c) => c.hashcat_line).join("\n");
    const blob = new Blob([content], { type: "text/plain" });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = `pmkid_${Date.now()}.22000`;
    a.click();
    URL.revokeObjectURL(url);
  }, [captures]);

  const columns = useMemo(() => [
    { key: "time", label: "Time", render: (r: PmkidRow) => <span className="text-muted-foreground">{formatTime(r.timestamp_ms)}</span> },
    { key: "bssid", label: "BSSID", render: (r: PmkidRow) => <span className="text-primary font-mono">{r.bssid}</span> },
    { key: "ssid", label: "SSID" },
    { key: "client_mac", label: "Client MAC", render: (r: PmkidRow) => <span className="font-mono">{r.client_mac}</span> },
    { key: "pmkid", label: "PMKID", render: (r: PmkidRow) => <span className="font-mono text-radar-yellow text-xs">{truncate(r.pmkid, 16)}</span> },
    {
      key: "actions", label: "", align: "right" as const,
      render: (r: PmkidRow) => (
        <button
          onClick={(e) => { e.stopPropagation(); copyHashcat(r); }}
          className="flex items-center gap-1 text-xs font-mono text-primary hover:text-primary/80 transition-colors"
        >
          <Copy className="w-3 h-3" />
          {copied === r._id ? "Copied!" : "Hashcat"}
        </button>
      ),
    },
  ], [copied, copyHashcat]);

  return (
    <div className="flex flex-col gap-6 flex-1 min-h-0">
      <div className="flex items-center gap-4">
        <ActionButton variant={running ? "destructive" : "primary"} onClick={toggle} loading={loading}>
          {running ? <><Square className="w-4 h-4" /> Stop Capture</> : <><Play className="w-4 h-4" /> Start Capture</>}
        </ActionButton>
        <StatusBadge status={running ? "active" : "inactive"} label={running ? "Capturing" : "Idle"} pulse />
        {captures.length > 0 && (
          <ActionButton variant="ghost" size="sm" onClick={exportAll}>
            <Download className="w-4 h-4" /> Export .22000
          </ActionButton>
        )}
      </div>

      <StatCard label="PMKID Captures" value={captures.length} accent="green" />

      {captures.length === 0 ? (
        <EmptyState icon={<KeyRound className="w-12 h-12" />} title="No PMKID Captures" description="Start PMKID capture to intercept EAPOL handshakes" />
      ) : (
        <DataTable columns={columns} data={captures as unknown as PmkidRow[]} keyField="_id" maxHeight="calc(100vh - 380px)" />
      )}
    </div>
  );
}

/* ─────────────────────── Pwnagotchi Tab ─────────────────────── */

function PwnagotchiTab({ interfaceName, onError }: SnifferControlProps) {
  const [detections, setDetections] = useState<(PwnagotchiInfo & { _id: number })[]>([]);
  const [running, setRunning] = useState(false);
  const [loading, setLoading] = useState(false);
  const idRef = useRef(0);

  useEffect(() => {
    let unlisten: UnlistenFn | null = null;
    let mounted = true;

    (async () => {
      unlisten = await listen<PwnagotchiInfo>("pwnagotchi-detected", (evt) => {
        if (!mounted) return;
        const id = ++idRef.current;
        setDetections((prev) => {
          const next = [{ ...evt.payload, _id: id }, ...prev];
          return next.length > MAX_ENTRIES ? next.slice(0, MAX_ENTRIES) : next;
        });
      });
    })();

    return () => {
      mounted = false;
      unlisten?.();
    };
  }, []);

  const toggle = async () => {
    setLoading(true);
    onError(null);
    try {
      if (running) {
        await invoke("stop_pwnagotchi_detect");
        setRunning(false);
      } else {
        await invoke("detect_pwnagotchi", { interfaceName });
        setRunning(true);
      }
    } catch (error) {
      onError(getErrorMessage(error));
    }
    setLoading(false);
  };

  const formatUptime = (secs: number): string => {
    const h = Math.floor(secs / 3600);
    const m = Math.floor((secs % 3600) / 60);
    const s = secs % 60;
    return `${h}h ${m}m ${s}s`;
  };

  return (
    <div className="flex flex-col gap-6 flex-1 min-h-0">
      <div className="flex items-center gap-4">
        <ActionButton variant={running ? "destructive" : "primary"} onClick={toggle} loading={loading}>
          {running ? <><Square className="w-4 h-4" /> Stop Detection</> : <><Play className="w-4 h-4" /> Start Detection</>}
        </ActionButton>
        <StatusBadge status={running ? "active" : "inactive"} label={running ? "Scanning" : "Idle"} pulse />
      </div>

      {detections.length === 0 ? (
        <EmptyState icon={<Ghost className="w-12 h-12" />} title="No Pwnagotchi Detected" description="Start detection to scan for nearby pwnagotchi devices" />
      ) : (
        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
          {detections.map((pwn) => (
            <GlassCard key={pwn._id} accent="green" className="p-6">
              <div className="flex items-center gap-3 mb-4">
                <div className="p-2 rounded-lg bg-radar-green/10 border border-radar-green/20">
                  <Ghost className="w-6 h-6 text-radar-green" />
                </div>
                <div>
                  <h3 className="font-mono font-bold text-lg text-radar-green text-glow">{pwn.name}</h3>
                  <p className="font-mono text-xs text-muted-foreground">v{pwn.version}</p>
                </div>
              </div>

              <div className="space-y-2 font-mono text-xs">
                <div className="flex justify-between">
                  <span className="text-muted-foreground">Uptime</span>
                  <span className="text-foreground">{formatUptime(pwn.uptime)}</span>
                </div>
                <div className="flex justify-between">
                  <span className="text-muted-foreground">Epochs</span>
                  <span className="text-radar-yellow">{pwn.epoch}</span>
                </div>
                <div className="flex justify-between">
                  <span className="text-muted-foreground">BSSID</span>
                  <span className="text-primary">{pwn.bssid}</span>
                </div>
                <div className="flex justify-between">
                  <span className="text-muted-foreground">Channel</span>
                  <span className="text-foreground">{pwn.channel ?? "—"}</span>
                </div>
                <div className="flex justify-between">
                  <span className="text-muted-foreground">RSSI</span>
                  <span>{pwn.rssi != null ? <SignalBar rssi={pwn.rssi} /> : "—"}</span>
                </div>
                <div className="flex justify-between">
                  <span className="text-muted-foreground">Detected</span>
                  <span className="text-foreground">{formatTime(pwn.timestamp_ms)}</span>
                </div>
              </div>
            </GlassCard>
          ))}
        </div>
      )}
    </div>
  );
}

/* ─────────────────────── Raw Frames Tab ─────────────────────── */

function RawFramesTab({ interfaceName, onError }: SnifferControlProps) {
  const [frames, setFrames] = useState<(RawFrame & { _id: number })[]>([]);
  const [running, setRunning] = useState(false);
  const [loading, setLoading] = useState(false);
  const idRef = useRef(0);

  useEffect(() => {
    let unlisten: UnlistenFn | null = null;
    let mounted = true;

    void (async () => {
      unlisten = await listen<RawFrame>("raw-frame", (evt) => {
        if (!mounted) return;
        const id = ++idRef.current;
        setFrames((prev) => {
          const next = [{ ...evt.payload, _id: id }, ...prev];
          return next.length > MAX_ENTRIES ? next.slice(0, MAX_ENTRIES) : next;
        });
      });
    })();

    return () => {
      mounted = false;
      unlisten?.();
    };
  }, []);

  const toggle = async () => {
    setLoading(true);
    onError(null);
    try {
      if (running) {
        await invoke("stop_raw_capture");
        setRunning(false);
      } else {
        await invoke("start_raw_capture", { interfaceName });
        setRunning(true);
      }
    } catch (error) {
      onError(getErrorMessage(error));
    }
    setLoading(false);
  };

  type RawRow = RawFrame & { _id: number; [key: string]: unknown };
  const columns = useMemo(() => [
    { key: "time", label: "Time", render: (row: RawRow) => <span className="text-muted-foreground">{formatTime(row.timestamp_ms)}</span> },
    { key: "frame_type", label: "Type" },
    { key: "subtype", label: "Subtype" },
    { key: "addr2", label: "Source", render: (row: RawRow) => <span className="font-mono">{row.addr2 ?? "—"}</span> },
    { key: "addr1", label: "Dest", render: (row: RawRow) => <span className="font-mono">{row.addr1 ?? "—"}</span> },
    { key: "channel", label: "CH", align: "center" as const, render: (row: RawRow) => row.channel ?? "—" },
    { key: "size", label: "Bytes", align: "center" as const },
  ], []);

  return (
    <div className="flex flex-col gap-6 flex-1 min-h-0">
      <div className="flex items-center gap-4">
        <ActionButton variant={running ? "destructive" : "primary"} onClick={toggle} loading={loading}>
          {running ? <><Square className="w-4 h-4" /> Stop Capture</> : <><Play className="w-4 h-4" /> Start Capture</>}
        </ActionButton>
        <StatusBadge status={running ? "active" : "inactive"} label={running ? "Capturing" : "Idle"} pulse />
      </div>

      <StatCard label="Raw Frames" value={frames.length} accent="primary" />

      {frames.length === 0 ? (
        <EmptyState icon={<Radio className="w-12 h-12" />} title="No Raw Frames" description="Start raw capture to inspect live 802.11 traffic." />
      ) : (
        <DataTable columns={columns} data={frames as unknown as RawRow[]} keyField="_id" maxHeight="calc(100vh - 380px)" />
      )}
    </div>
  );
}

/* ─────────────────────── SAE Tab ─────────────────────── */

function SaeSniffTab({ interfaceName, onError }: SnifferControlProps) {
  const [frames, setFrames] = useState<(SaeFrame & { _id: number })[]>([]);
  const [running, setRunning] = useState(false);
  const [loading, setLoading] = useState(false);
  const idRef = useRef(0);

  useEffect(() => {
    let unlisten: UnlistenFn | null = null;
    let mounted = true;

    void (async () => {
      unlisten = await listen<SaeFrame>("sae-frame", (evt) => {
        if (!mounted) return;
        const id = ++idRef.current;
        setFrames((prev) => {
          const next = [{ ...evt.payload, _id: id }, ...prev];
          return next.length > MAX_ENTRIES ? next.slice(0, MAX_ENTRIES) : next;
        });
      });
    })();

    return () => {
      mounted = false;
      unlisten?.();
    };
  }, []);

  const toggle = async () => {
    setLoading(true);
    onError(null);
    try {
      if (running) {
        await invoke("stop_sae_sniff");
        setRunning(false);
      } else {
        await invoke("start_sae_sniff", { interfaceName });
        setRunning(true);
      }
    } catch (error) {
      onError(getErrorMessage(error));
    }
    setLoading(false);
  };

  type SaeRow = SaeFrame & { _id: number; [key: string]: unknown };
  const commitCount = useMemo(() => frames.filter((frame) => frame.is_commit).length, [frames]);
  const confirmCount = useMemo(() => frames.filter((frame) => frame.is_confirm).length, [frames]);
  const columns = useMemo(() => [
    { key: "time", label: "Time", render: (row: SaeRow) => <span className="text-muted-foreground">{formatTime(row.timestamp_ms)}</span> },
    { key: "source", label: "Source", render: (row: SaeRow) => <span className="font-mono text-primary">{row.source}</span> },
    { key: "destination", label: "Destination", render: (row: SaeRow) => <span className="font-mono">{row.destination}</span> },
    { key: "bssid", label: "BSSID", render: (row: SaeRow) => <span className="font-mono text-muted-foreground">{row.bssid}</span> },
    { key: "seq_num", label: "Seq", align: "center" as const },
    { key: "kind", label: "Kind", render: (row: SaeRow) => row.is_commit ? "Commit" : row.is_confirm ? "Confirm" : "Other" },
  ], []);

  return (
    <div className="flex flex-col gap-6 flex-1 min-h-0">
      <div className="flex items-center gap-4">
        <ActionButton variant={running ? "destructive" : "primary"} onClick={toggle} loading={loading}>
          {running ? <><Square className="w-4 h-4" /> Stop Sniff</> : <><Play className="w-4 h-4" /> Start Sniff</>}
        </ActionButton>
        <StatusBadge status={running ? "active" : "inactive"} label={running ? "Sniffing" : "Idle"} pulse />
      </div>

      <div className="grid grid-cols-3 gap-4">
        <StatCard label="SAE Frames" value={frames.length} accent="primary" />
        <StatCard label="Commits" value={commitCount} accent="yellow" />
        <StatCard label="Confirms" value={confirmCount} accent="green" />
      </div>

      {frames.length === 0 ? (
        <EmptyState icon={<KeyRound className="w-12 h-12" />} title="No SAE Frames" description="Start SAE sniff to monitor WPA3 authentication exchanges." />
      ) : (
        <DataTable columns={columns} data={frames as unknown as SaeRow[]} keyField="_id" maxHeight="calc(100vh - 420px)" />
      )}
    </div>
  );
}

/* ─────────────────────── MAC Track Tab ─────────────────────── */

function MacTrackTab({ interfaceName, onError }: SnifferControlProps) {
  const [targetMac, setTargetMac] = useState("");
  const [entries, setEntries] = useState<(MacTrackEntry & { _id: number })[]>([]);
  const [running, setRunning] = useState(false);
  const [loading, setLoading] = useState(false);
  const idRef = useRef(0);

  useEffect(() => {
    let unlisten: UnlistenFn | null = null;
    let mounted = true;

    void (async () => {
      unlisten = await listen<MacTrackEntry>("mac-track", (evt) => {
        if (!mounted) return;
        const id = ++idRef.current;
        setEntries((prev) => {
          const next = [{ ...evt.payload, _id: id }, ...prev];
          return next.length > MAX_ENTRIES ? next.slice(0, MAX_ENTRIES) : next;
        });
      });
    })();

    return () => {
      mounted = false;
      unlisten?.();
    };
  }, []);

  const toggle = async () => {
    setLoading(true);
    onError(null);
    try {
      if (running) {
        await invoke("stop_mac_track");
        setRunning(false);
      } else {
        await invoke("start_mac_track", { interfaceName, targetMac });
        setRunning(true);
      }
    } catch (error) {
      onError(getErrorMessage(error));
    }
    setLoading(false);
  };

  type MacTrackRow = MacTrackEntry & { _id: number; [key: string]: unknown };
  const columns = useMemo(() => [
    { key: "time", label: "Time", render: (row: MacTrackRow) => <span className="text-muted-foreground">{formatTime(row.timestamp_ms)}</span> },
    { key: "mac", label: "MAC", render: (row: MacTrackRow) => <span className="font-mono text-primary">{row.mac}</span> },
    { key: "role", label: "Role" },
    { key: "frame_type", label: "Frame" },
    { key: "channel", label: "CH", align: "center" as const, render: (row: MacTrackRow) => row.channel ?? "—" },
    { key: "rssi", label: "RSSI", render: (row: MacTrackRow) => row.rssi == null ? "—" : <SignalBar rssi={row.rssi} /> },
  ], []);

  return (
    <div className="flex flex-col gap-6 flex-1 min-h-0">
      <GlassCard className="p-4">
        <div className="flex flex-wrap items-end gap-4">
          <InputField label="Target MAC" value={targetMac} onChange={(value) => setTargetMac(value.toUpperCase())} placeholder="AA:BB:CC:DD:EE:FF" className="flex-1 min-w-[260px]" />
          <ActionButton variant={running ? "destructive" : "primary"} onClick={toggle} loading={loading} disabled={!running && !targetMac.trim()}>
            {running ? <><Square className="w-4 h-4" /> Stop Tracking</> : <><Play className="w-4 h-4" /> Start Tracking</>}
          </ActionButton>
        </div>
      </GlassCard>

      <StatCard label="MAC Hits" value={entries.length} accent="primary" />

      {entries.length === 0 ? (
        <EmptyState icon={<Eye className="w-12 h-12" />} title="No MAC Hits" description="Track a MAC address to see where it appears in live traffic." />
      ) : (
        <DataTable columns={columns} data={entries as unknown as MacTrackRow[]} keyField="_id" maxHeight="calc(100vh - 420px)" />
      )}
    </div>
  );
}

/* ─────────────────────── Main Sniffer Page ─────────────────────── */

const TABS = [
  { id: "probes", label: "Probe Requests", icon: <Radio className="w-3.5 h-3.5" /> },
  { id: "deauth", label: "Deauth Monitor", icon: <ShieldAlert className="w-3.5 h-3.5" /> },
  { id: "packets", label: "Packet Stats", icon: <BarChart3 className="w-3.5 h-3.5" /> },
  { id: "raw", label: "Raw Frames", icon: <Eye className="w-3.5 h-3.5" /> },
  { id: "pmkid", label: "PMKID", icon: <KeyRound className="w-3.5 h-3.5" /> },
  { id: "sae", label: "SAE", icon: <KeyRound className="w-3.5 h-3.5" /> },
  { id: "mac-track", label: "MAC Track", icon: <Eye className="w-3.5 h-3.5" /> },
  { id: "pwnagotchi", label: "Pwnagotchi", icon: <Ghost className="w-3.5 h-3.5" /> },
];

export function Sniffer() {
  const [tab, setTab] = useState("probes");
  const [interfaceName, setInterfaceName] = useState("wlan0");
  const [error, setError] = useState<string | null>(null);

  return (
    <div className="h-full flex flex-col animate-in fade-in slide-in-from-bottom-4 duration-500">
      <PageHeader
        icon={<Eye className="w-8 h-8" />}
        title="SNIFFER"
        subtitle="PASSIVE INTELLIGENCE"
      />

      {error && <ErrorBanner message={error} onDismiss={() => setError(null)} />}

      <GlassCard className="mb-4 p-4">
        <div className="flex flex-wrap items-end gap-4">
          <InputField
            label="Interface"
            value={interfaceName}
            onChange={setInterfaceName}
            placeholder="wlan0"
            className="min-w-[220px]"
          />
          <span className="font-mono text-xs uppercase tracking-widest text-muted-foreground">
            Shared across all sniffer tabs
          </span>
        </div>
      </GlassCard>

      <TabBar tabs={TABS} active={tab} onChange={setTab} />

      <div className="flex-1 min-h-0 flex flex-col">
        <div className={tab === "probes" ? "flex-1 flex flex-col" : "hidden"}>
          <ProbeRequestsTab interfaceName={interfaceName} onError={setError} />
        </div>
        <div className={tab === "deauth" ? "flex-1 flex flex-col" : "hidden"}>
          <DeauthMonitorTab interfaceName={interfaceName} onError={setError} />
        </div>
        <div className={tab === "packets" ? "flex-1 flex flex-col" : "hidden"}>
          <PacketStatsTab interfaceName={interfaceName} onError={setError} />
        </div>
        <div className={tab === "raw" ? "flex-1 flex flex-col" : "hidden"}>
          <RawFramesTab interfaceName={interfaceName} onError={setError} />
        </div>
        <div className={tab === "pmkid" ? "flex-1 flex flex-col" : "hidden"}>
          <PmkidTab interfaceName={interfaceName} onError={setError} />
        </div>
        <div className={tab === "sae" ? "flex-1 flex flex-col" : "hidden"}>
          <SaeSniffTab interfaceName={interfaceName} onError={setError} />
        </div>
        <div className={tab === "mac-track" ? "flex-1 flex flex-col" : "hidden"}>
          <MacTrackTab interfaceName={interfaceName} onError={setError} />
        </div>
        <div className={tab === "pwnagotchi" ? "flex-1 flex flex-col" : "hidden"}>
          <PwnagotchiTab interfaceName={interfaceName} onError={setError} />
        </div>
      </div>
    </div>
  );
}
