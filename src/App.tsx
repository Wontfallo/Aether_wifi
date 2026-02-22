import { Routes, Route } from "react-router-dom";
import { AppShell } from "./components/layout/AppShell";
import { Dashboard } from "./pages/Dashboard";
import { Spectrum } from "./pages/Spectrum";
import { Hunt } from "./pages/Hunt";
import { Audit } from "./pages/Audit";
import { EnvironmentDoctor } from "./pages/EnvironmentDoctor";
import { SettingsPage } from "./pages/Settings";
import { BeaconCaptureProvider } from "./hooks/useBeaconCapture";

function App() {
  return (
    <BeaconCaptureProvider>
      <AppShell>
        <Routes>
          <Route path="/" element={<Dashboard />} />
          <Route path="/spectrum" element={<Spectrum />} />
          <Route path="/hunt" element={<Hunt />} />
          <Route path="/audit" element={<Audit />} />
          <Route path="/doctor" element={<EnvironmentDoctor />} />
          <Route path="/settings" element={<SettingsPage />} />
        </Routes>
      </AppShell>
    </BeaconCaptureProvider>
  );
}

export default App;
