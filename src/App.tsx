import { Routes, Route, Navigate } from "react-router-dom";
import { AppShell } from "./components/layout/AppShell";
import { Dashboard } from "./pages/Dashboard";
import { Spectrum } from "./pages/Spectrum";
import { Hunt } from "./pages/Hunt";
import { Strike } from "./pages/Strike";
import { EnvironmentDoctor } from "./pages/EnvironmentDoctor";
import { SettingsPage } from "./pages/Settings";
import { Recon } from "./pages/Recon";
import { Sniffer } from "./pages/Sniffer";
import { Tools } from "./pages/Tools";
import { BeaconCaptureProvider } from "./hooks/useBeaconCapture";

function App() {
  return (
    <BeaconCaptureProvider>
      <AppShell>
        <Routes>
          <Route path="/" element={<Dashboard />} />
          <Route path="/spectrum" element={<Spectrum />} />
          <Route path="/hunt" element={<Hunt />} />
          <Route path="/strike" element={<Strike />} />
          <Route path="/offensive" element={<Navigate to="/strike" replace />} />
          <Route path="/audit" element={<Navigate to="/strike" replace />} />
          <Route path="/attack" element={<Navigate to="/strike" replace />} />
          <Route path="/recon" element={<Recon />} />
          <Route path="/sniffer" element={<Sniffer />} />
          <Route path="/tools" element={<Tools />} />
          <Route path="/doctor" element={<EnvironmentDoctor />} />
          <Route path="/settings" element={<SettingsPage />} />
        </Routes>
      </AppShell>
    </BeaconCaptureProvider>
  );
}

export default App;
