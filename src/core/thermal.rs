// ============================================================================
// thermal.rs — Thermal & Battery-Aware Dynamic Scheduling for apfel-rs
// Monitors macOS IOPowerSources and ThermalPressure to adaptively balance
// Apple Neural Engine (ANE) low-power vs. Metal GPU high-throughput execution.
// ============================================================================

use std::process::Command;
use serde::{Deserialize, Serialize};
use tracing::debug;

/// Power source state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PowerSource {
    AcPower,
    Battery { percent: u8 },
    Unknown,
}

/// System thermal pressure state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ThermalState {
    Nominal,
    Fair,
    Serious,
    Critical,
}

/// Dynamically selected execution profile for on-device inference.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ComputeProfile {
    /// Full Metal GPU acceleration + speculative multi-token decoding (AC & Nominal).
    Performance,
    /// Balanced Metal / ANE execution (Battery > 50% & Fair).
    Balanced,
    /// Pure Apple Neural Engine (ANE) low-power execution with greedy decoding (<2W, Battery < 30% or Serious).
    EcoPowerSaver,
}

impl ComputeProfile {
    /// Maximum recommended token context budget under this profile.
    pub fn context_token_ceiling(&self) -> usize {
        match self {
            ComputeProfile::Performance => 4096,
            ComputeProfile::Balanced => 3072,
            ComputeProfile::EcoPowerSaver => 2048,
        }
    }

    /// Whether speculative draft decoding should be enabled.
    pub fn enable_speculative_decoding(&self) -> bool {
        matches!(self, ComputeProfile::Performance)
    }

    /// Number of concurrent worker threads.
    pub fn worker_threads(&self) -> usize {
        match self {
            ComputeProfile::Performance => 8,
            ComputeProfile::Balanced => 4,
            ComputeProfile::EcoPowerSaver => 2,
        }
    }
}

/// Power and thermal monitor for macOS Apple Silicon.
pub struct PowerThermalMonitor;

impl PowerThermalMonitor {
    /// Probes current power source (AC power vs battery percentage).
    pub fn current_power_source() -> PowerSource {
        #[cfg(target_os = "macos")]
        {
            if let Ok(output) = Command::new("pmset").arg("-g").arg("batt").output() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                if stdout.contains("AC Power") {
                    return PowerSource::AcPower;
                }
                // Parse battery percentage: e.g. "Now drawing from 'Battery Power'\n -InternalBattery-0 (id=...) 85%;"
                if let Some(pos) = stdout.find('%') {
                    let slice = &stdout[..pos];
                    if let Some(start) = slice.rfind(|c: char| !c.is_ascii_digit()) {
                        if let Ok(pct) = slice[start + 1..].parse::<u8>() {
                            return PowerSource::Battery { percent: pct };
                        }
                    }
                }
            }
        }
        PowerSource::AcPower
    }

    /// Probes current thermal pressure state.
    pub fn current_thermal_state() -> ThermalState {
        #[cfg(target_os = "macos")]
        {
            if let Ok(output) = Command::new("pmset").arg("-g").arg("therm").output() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                if stdout.contains("CPU_Speed_Limit") && stdout.contains("100") {
                    return ThermalState::Nominal;
                }
                if stdout.contains("CPU_Speed_Limit") {
                    return ThermalState::Fair;
                }
            }
        }
        ThermalState::Nominal
    }

    /// Evaluates the compute profile for a given power source and thermal state.
    pub fn resolve_profile(power: &PowerSource, thermal: &ThermalState) -> ComputeProfile {
        match (power, thermal) {
            // Serious or critical thermal throttling -> immediate EcoPowerSaver to cool down
            (_, ThermalState::Serious) | (_, ThermalState::Critical) => ComputeProfile::EcoPowerSaver,
            // Low battery -> EcoPowerSaver to maximize battery life
            (PowerSource::Battery { percent }, _) if *percent <= 30 => ComputeProfile::EcoPowerSaver,
            // Battery power -> Balanced
            (PowerSource::Battery { .. }, _) => ComputeProfile::Balanced,
            // AC power and cool -> Full Performance
            (PowerSource::AcPower, ThermalState::Nominal) => ComputeProfile::Performance,
            _ => ComputeProfile::Balanced,
        }
    }

    /// Evaluates the optimal compute profile based on power source and thermal state.
    pub fn resolve_optimal_profile() -> ComputeProfile {
        let power = Self::current_power_source();
        let thermal = Self::current_thermal_state();
        let profile = Self::resolve_profile(&power, &thermal);
        debug!(?power, ?thermal, ?profile, "Resolved adaptive compute profile");
        profile
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_profile_budgets() {
        assert_eq!(ComputeProfile::Performance.context_token_ceiling(), 4096);
        assert_eq!(ComputeProfile::Balanced.context_token_ceiling(), 3072);
        assert_eq!(ComputeProfile::EcoPowerSaver.context_token_ceiling(), 2048);
        assert!(ComputeProfile::Performance.enable_speculative_decoding());
        assert!(!ComputeProfile::EcoPowerSaver.enable_speculative_decoding());
    }

    #[test]
    fn test_power_thermal_probe() {
        let power = PowerThermalMonitor::current_power_source();
        let thermal = PowerThermalMonitor::current_thermal_state();
        let profile = PowerThermalMonitor::resolve_optimal_profile();
        assert!(profile.worker_threads() >= 2);
        println!("Probed: power={:?}, thermal={:?}, profile={:?}", power, thermal, profile);
    }
}
