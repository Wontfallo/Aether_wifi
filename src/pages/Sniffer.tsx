import { Eye, Play, Square, Radio, ShieldAlert, BarChart3, KeyRound, Ghost, Copy, Download } from "lucide-react";
import ReactECharts from "echarts-for-react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import {
  PageHeader, ActionButton, StatCard, DataTable,
  TabBar, EmptyState, GlassCard, StatusBadge, SignalBar,
} from "../components/ui/shared";
import type {
  ProbeRequest, DeauthEvent, PacketStats, PmkidCapture, PwnagotchiInfo,
} from "../types/capture";

const MAX_ENTRIES = 500;
const FPS_HISTORY = 60;

function formatTime(ms: number): string {
  const d = new Date(ms);
  return d.toLocaleTimeString("en-GB", { hour12: false });
}

function truncate(s: string, len: number): string {
  return s.length > len ? s.slice(0, len) + "…" : s;
}

/* ─────────────────────── Probe Requests Tab ─────────────────────── */

function ProbeRequestsTab() {
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
    try {
      if (running) {
        await invoke("stop_sniffer");
        setRunning(false);
      } else {
        await invoke("start_sniffer", { interfaceName: "wlan0" });
        setRunning(true);
      }
    } catch { /* ignore */ }
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

function DeauthMonitorTab() {
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
    try {
      if (running) {
        await invoke("stop_sniffer");
        setRunning(false);
      } else {
        await invoke("start_sniffer", { interfaceName: "wlan0" });
        setRunning(true);
      }
    } catch { /* ignore */ }
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

function PacketStatsTab() {
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
    try {
      if (running) {
        await invoke("stop_packet_monitor");
        setRunning(false);
      } else {
        await invoke("start_packet_monitor", { interfaceName: "wlan0" });
        setRunning(true);
      }
    } catch { /* ignore */ }
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

function PmkidTab() {
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
    try {
      if (running) {
        await invoke("stop_pmkid_capture");
        setRunning(false);
      } else {
        await invoke("start_pmkid_capture", { interfaceName: "wlan0" });
        setRunning(true);
      }
    } catch { /* ignore */ }
    setLoading(false);
  };

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

  type PmkidRow = PmkidCapture & { _id: number; [key: string]: unknown };
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

function PwnagotchiTab() {
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
    try {
      if (running) {
        await invoke("stop_pwnagotchi_detect");
        setRunning(false);
      } else {
        await invoke("detect_pwnagotchi", { interfaceName: "wlan0" });
        setRunning(true);
      }
    } catch { /* ignore */ }
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

/* ─────────────────────── Main Sniffer Page ─────────────────────── */

const TABS = [
  { id: "probes", label: "Probe Requests", icon: <Radio className="w-3.5 h-3.5" /> },
  { id: "deauth", label: "Deauth Monitor", icon: <ShieldAlert className="w-3.5 h-3.5" /> },
  { id: "packets", label: "Packet Stats", icon: <BarChart3 className="w-3.5 h-3.5" /> },
  { id: "pmkid", label: "PMKID", icon: <KeyRound className="w-3.5 h-3.5" /> },
  { id: "pwnagotchi", label: "Pwnagotchi", icon: <Ghost className="w-3.5 h-3.5" /> },
];

export function Sniffer() {
  const [tab, setTab] = useState("probes");

  return (
    <div className="h-full flex flex-col animate-in fade-in slide-in-from-bottom-4 duration-500">
      <PageHeader
        icon={<Eye className="w-8 h-8" />}
        title="SNIFFER"
        subtitle="PASSIVE INTELLIGENCE"
      />

      <TabBar tabs={TABS} active={tab} onChange={setTab} />

      <div className="flex-1 min-h-0 flex flex-col">
        {tab === "probes" && <ProbeRequestsTab />}
        {tab === "deauth" && <DeauthMonitorTab />}
        {tab === "packets" && <PacketStatsTab />}
        {tab === "pmkid" && <PmkidTab />}
        {tab === "pwnagotchi" && <PwnagotchiTab />}
      </div>
    </div>
  );
}
