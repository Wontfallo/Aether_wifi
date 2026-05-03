import { Radar, Search, Server } from "lucide-react";
import { useState, useCallback, useEffect, type ReactNode } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { HostInfo, NetworkInterface, PortResult, ServiceInfo } from "../types/capture";
import {
  PageHeader,
  TabBar,
  GlassCard,
  StatCard,
  ActionButton,
  DataTable,
  InputField,
  SelectField,
} from "../components/ui/shared";

// eslint-disable-next-line @typescript-eslint/no-explicit-any
type AnyRecord = Record<string, any>;

interface Column {
  key: string;
  label: string;
  align?: "left" | "center" | "right";
  render?: (item: AnyRecord) => ReactNode;
}

type TabId = "hosts" | "ports" | "services";

const tabs: { id: TabId; label: string; icon?: React.ReactNode }[] = [
  { id: "hosts", label: "Host Discovery", icon: <Search className="w-3.5 h-3.5" /> },
  { id: "ports", label: "Port Scan", icon: <Server className="w-3.5 h-3.5" /> },
  { id: "services", label: "Services", icon: <Radar className="w-3.5 h-3.5" /> },
];

/* ─── State colors ─── */
const portStateColor: Record<string, string> = {
  open: "text-radar-green",
  filtered: "text-radar-yellow",
  closed: "text-destructive",
};

/* ─── Column definitions ─── */
const hostColumns: Column[] = [
  { key: "ip", label: "IP" },
  { key: "mac", label: "MAC", render: (h) => (h.mac as string) ?? "—" },
  { key: "hostname", label: "Hostname", render: (h) => (h.hostname as string) ?? "—" },
  { key: "vendor", label: "Vendor", render: (h) => (h.vendor as string) ?? "—" },
  {
    key: "is_up",
    label: "Status",
    align: "center",
    render: (h) => (
      <span className="inline-flex items-center gap-1.5">
        <span className={`inline-block w-2 h-2 rounded-full ${h.is_up ? "bg-radar-green" : "bg-destructive"}`} />
        <span className="uppercase text-[11px] tracking-wider">{h.is_up ? "Up" : "Down"}</span>
      </span>
    ),
  },
];

const portColumns: Column[] = [
  { key: "host", label: "Host" },
  { key: "port", label: "Port", align: "center" },
  { key: "protocol", label: "Protocol", align: "center", render: (p) => (p.protocol as string).toUpperCase() },
  {
    key: "state",
    label: "State",
    align: "center",
    render: (p) => (
      <span className={`uppercase font-bold text-[11px] tracking-wider ${portStateColor[p.state as string] ?? "text-muted-foreground"}`}>
        {p.state as string}
      </span>
    ),
  },
  { key: "service", label: "Service", render: (p) => (p.service as string) ?? "—" },
  { key: "version", label: "Version", render: (p) => (p.version as string) ?? "—" },
];

const serviceColumns: Column[] = [
  { key: "host", label: "Host" },
  { key: "port", label: "Port", align: "center" },
  { key: "service", label: "Service" },
  { key: "version", label: "Version", render: (s) => (s.version as string) ?? "—" },
  { key: "mac", label: "MAC", render: (s) => (s.mac as string) ?? "—" },
  { key: "vendor", label: "Vendor", render: (s) => (s.vendor as string) ?? "—" },
];

/* ─── Error banner ─── */
function ErrorBanner({ message, onDismiss }: { message: string; onDismiss: () => void }) {
  return (
    <div className="mb-4 p-4 border border-destructive/50 bg-destructive/10 text-destructive rounded-lg font-mono text-sm flex items-center justify-between gap-4">
      <span className="break-all">{message}</span>
      <button onClick={onDismiss} className="shrink-0 opacity-70 hover:opacity-100 transition-opacity text-lg leading-none">×</button>
    </div>
  );
}

/* ─── Helpers ─── */
function parseError(err: unknown): string {
  if (typeof err === "string") return err;
  if (err instanceof Error) return err.message;
  return JSON.stringify(err);
}

/* ─── Page ─── */
export function Recon() {
  const [activeTab, setActiveTab] = useState<TabId>("hosts");

  // Host Discovery state
  const [hostSubnet, setHostSubnet] = useState("192.168.1.0/24");
  const [hostInterface, setHostInterface] = useState("eth0");
  const [interfaces, setInterfaces] = useState<NetworkInterface[]>([]);
  const [hosts, setHosts] = useState<HostInfo[]>([]);
  const [hostLoading, setHostLoading] = useState<"ping" | "arp" | null>(null);
  const [hostError, setHostError] = useState<string | null>(null);

  // Port Scan state
  const [portTarget, setPortTarget] = useState("");
  const [portRange, setPortRange] = useState("1-1000");
  const [ports, setPorts] = useState<PortResult[]>([]);
  const [portLoading, setPortLoading] = useState(false);
  const [portError, setPortError] = useState<string | null>(null);

  // Services state
  const [svcSubnet, setSvcSubnet] = useState("192.168.1.0/24");
  const [services, setServices] = useState<ServiceInfo[]>([]);
  const [svcLoading, setSvcLoading] = useState<"ssh" | "telnet" | null>(null);
  const [svcError, setSvcError] = useState<string | null>(null);

  useEffect(() => {
    let mounted = true;

    void (async () => {
      try {
        const result = await invoke<NetworkInterface[]>("list_interfaces");
        if (!mounted) return;
        setInterfaces(result);
        if (!result.some((item) => item.name === hostInterface) && result[0]) {
          setHostInterface(result[0].name);
        }
      } catch {
        // Keep manual entry fallback if interface discovery fails.
      }
    })();

    return () => {
      mounted = false;
    };
  }, [hostInterface]);

  /* ─── Host Discovery handlers ─── */
  const handlePingScan = useCallback(async () => {
    setHostError(null);
    setHostLoading("ping");
    try {
      const result = await invoke<HostInfo[]>("ping_scan", { subnet: hostSubnet });
      setHosts(result);
    } catch (err: unknown) {
      setHostError(parseError(err));
    } finally {
      setHostLoading(null);
    }
  }, [hostSubnet]);

  const handleArpScan = useCallback(async () => {
    setHostError(null);
    setHostLoading("arp");
    try {
      const result = await invoke<HostInfo[]>("arp_scan", { interfaceName: hostInterface });
      setHosts(result);
    } catch (err: unknown) {
      setHostError(parseError(err));
    } finally {
      setHostLoading(null);
    }
  }, [hostInterface]);

  /* ─── Port Scan handler ─── */
  const handlePortScan = useCallback(async () => {
    setPortError(null);
    setPortLoading(true);
    try {
      const result = await invoke<PortResult[]>("port_scan", { target: portTarget, ports: portRange });
      setPorts(result);
    } catch (err: unknown) {
      setPortError(parseError(err));
    } finally {
      setPortLoading(false);
    }
  }, [portTarget, portRange]);

  /* ─── Services handlers ─── */
  const handleSshScan = useCallback(async () => {
    setSvcError(null);
    setSvcLoading("ssh");
    try {
      const result = await invoke<ServiceInfo[]>("ssh_scan", { subnet: svcSubnet });
      setServices(result);
    } catch (err: unknown) {
      setSvcError(parseError(err));
    } finally {
      setSvcLoading(null);
    }
  }, [svcSubnet]);

  const handleTelnetScan = useCallback(async () => {
    setSvcError(null);
    setSvcLoading("telnet");
    try {
      const result = await invoke<ServiceInfo[]>("telnet_scan", { subnet: svcSubnet });
      setServices(result);
    } catch (err: unknown) {
      setSvcError(parseError(err));
    } finally {
      setSvcLoading(null);
    }
  }, [svcSubnet]);

  /* ─── Computed stats ─── */
  const upHosts = hosts.filter((h) => h.is_up).length;
  const downHosts = hosts.filter((h) => !h.is_up).length;

  return (
    <div className="h-full flex flex-col animate-in fade-in slide-in-from-bottom-4 duration-500">
      <PageHeader icon={<Radar className="w-6 h-6" />} title="RECON" subtitle="NETWORK DISCOVERY" />

      <div className="mb-6">
        <TabBar tabs={tabs} active={activeTab} onChange={(id) => setActiveTab(id as TabId)} />
      </div>

      <div className="flex-1 min-h-0 flex flex-col gap-6">
        {/* ═══════ Host Discovery ═══════ */}
        {activeTab === "hosts" && (
          <>
            {hostError && <ErrorBanner message={hostError} onDismiss={() => setHostError(null)} />}

            <GlassCard className="p-5">
              <div className="flex flex-wrap items-end gap-4">
                <InputField label="Subnet" value={hostSubnet} onChange={setHostSubnet} placeholder="192.168.1.0/24" mono className="flex-1 min-w-[200px]" />
                {interfaces.length > 0 ? (
                  <SelectField
                    label="Interface (ARP)"
                    value={hostInterface}
                    onChange={setHostInterface}
                    options={interfaces.map((item) => ({ value: item.name, label: item.name }))}
                    className="flex-1 min-w-[140px]"
                  />
                ) : (
                  <InputField label="Interface (ARP)" value={hostInterface} onChange={setHostInterface} placeholder="eth0" mono className="flex-1 min-w-[140px]" />
                )}
                <div className="flex gap-2">
                  <ActionButton variant="primary" size="md" onClick={handlePingScan} loading={hostLoading === "ping"} disabled={hostLoading !== null}>
                    Ping Scan
                  </ActionButton>
                  <ActionButton variant="primary" size="md" onClick={handleArpScan} loading={hostLoading === "arp"} disabled={hostLoading !== null}>
                    ARP Scan
                  </ActionButton>
                </div>
              </div>
            </GlassCard>

            <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
              <StatCard label="Total Hosts" value={hosts.length} accent="primary" />
              <StatCard label="Up" value={upHosts} accent="green" />
              <StatCard label="Down" value={downHosts} accent="destructive" />
            </div>

            <GlassCard accent="none" className="flex-1 min-h-0 overflow-auto">
              <DataTable
                columns={hostColumns}
                data={hosts as unknown as AnyRecord[]}
                keyField="ip"
                emptyMessage="No hosts discovered. Run a scan to begin."
              />
            </GlassCard>
          </>
        )}

        {/* ═══════ Port Scan ═══════ */}
        {activeTab === "ports" && (
          <>
            {portError && <ErrorBanner message={portError} onDismiss={() => setPortError(null)} />}

            <GlassCard className="p-5">
              <div className="flex flex-wrap items-end gap-4">
                <InputField label="Target IP" value={portTarget} onChange={setPortTarget} placeholder="192.168.1.1" mono className="flex-1 min-w-[200px]" />
                <InputField label="Port Range" value={portRange} onChange={setPortRange} placeholder="1-1000" mono className="flex-1 min-w-[140px]" />
                <ActionButton variant="primary" size="md" onClick={handlePortScan} loading={portLoading} disabled={portLoading || !portTarget}>
                  Scan Ports
                </ActionButton>
              </div>
            </GlassCard>

            <GlassCard accent="none" className="flex-1 min-h-0 overflow-auto">
              <DataTable
                columns={portColumns}
                data={ports as unknown as AnyRecord[]}
                keyField="port"
                emptyMessage="No port scan results. Enter a target and scan."
              />
            </GlassCard>
          </>
        )}

        {/* ═══════ Services ═══════ */}
        {activeTab === "services" && (
          <>
            {svcError && <ErrorBanner message={svcError} onDismiss={() => setSvcError(null)} />}

            <GlassCard className="p-5">
              <div className="flex flex-wrap items-end gap-4">
                <InputField label="Subnet" value={svcSubnet} onChange={setSvcSubnet} placeholder="192.168.1.0/24" mono className="flex-1 min-w-[200px]" />
                <div className="flex gap-2">
                  <ActionButton variant="primary" size="md" onClick={handleSshScan} loading={svcLoading === "ssh"} disabled={svcLoading !== null}>
                    SSH Scan
                  </ActionButton>
                  <ActionButton variant="primary" size="md" onClick={handleTelnetScan} loading={svcLoading === "telnet"} disabled={svcLoading !== null}>
                    Telnet Scan
                  </ActionButton>
                </div>
              </div>
            </GlassCard>

            <GlassCard accent="none" className="flex-1 min-h-0 overflow-auto">
              <DataTable
                columns={serviceColumns}
                data={services as unknown as AnyRecord[]}
                keyField="port"
                emptyMessage="No services found. Run a service scan."
              />
            </GlassCard>
          </>
        )}
      </div>
    </div>
  );
}
