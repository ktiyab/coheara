use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Type of vital sign measurement (Spec 47: Feature Enhancements).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VitalType {
    Temperature,
    BloodPressure,
    Weight,
    Height,
    HeartRate,
    BloodGlucose,
    OxygenSaturation,
}

impl VitalType {
    pub fn as_str(self) -> &'static str {
        match self {
            VitalType::Temperature => "temperature",
            VitalType::BloodPressure => "blood_pressure",
            VitalType::Weight => "weight",
            VitalType::Height => "height",
            VitalType::HeartRate => "heart_rate",
            VitalType::BloodGlucose => "blood_glucose",
            VitalType::OxygenSaturation => "oxygen_saturation",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "temperature" => Some(VitalType::Temperature),
            "blood_pressure" => Some(VitalType::BloodPressure),
            "weight" => Some(VitalType::Weight),
            "height" => Some(VitalType::Height),
            "heart_rate" => Some(VitalType::HeartRate),
            "blood_glucose" => Some(VitalType::BloodGlucose),
            "oxygen_saturation" => Some(VitalType::OxygenSaturation),
            _ => None,
        }
    }

    /// Default unit for this vital type.
    pub fn default_unit(self) -> &'static str {
        match self {
            VitalType::Temperature => "°C",
            VitalType::BloodPressure => "mmHg",
            VitalType::Weight => "kg",
            VitalType::Height => "cm",
            VitalType::HeartRate => "bpm",
            VitalType::BloodGlucose => "mg/dL",
            VitalType::OxygenSaturation => "%",
        }
    }
}

/// Source of the vital sign measurement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VitalSource {
    Manual,
    Imported,
}

impl VitalSource {
    pub fn as_str(self) -> &'static str {
        match self {
            VitalSource::Manual => "manual",
            VitalSource::Imported => "imported",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "manual" => Some(VitalSource::Manual),
            "imported" => Some(VitalSource::Imported),
            _ => None,
        }
    }
}

/// A single vital sign measurement.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VitalSign {
    pub id: Uuid,
    pub vital_type: VitalType,
    pub value_primary: f64,
    pub value_secondary: Option<f64>, // diastolic for blood_pressure
    pub unit: String,
    pub recorded_at: NaiveDateTime,
    pub notes: Option<String>,
    pub source: VitalSource,
    pub created_at: NaiveDateTime,
}
