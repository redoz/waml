import type { ModelGraph } from "@mc/okf";
import { f, mart, rel, type Template } from "./helpers";

// Healthcare provider — operational + revenue-cycle model. fct_appointments
// carries scheduling (no-show, wait, lead time); fct_encounters the clinical
// visit (LOS, 30-day readmission); fct_claims the revenue cycle (denials, AR
// days) against the payer dimension. Patient and provider are conformed dims.
//
// Goal coverage (niche "healthcare"):
//   30-day readmission      → fct_encounters.is_readmission_30d × dim_patient.risk_tier
//   no-show rate            → fct_appointments (is_no_show, lead_time_days)
//   AR days & denials       → fct_claims (ar_days, denial_code) × dim_payer
//   LOS & bed utilization   → fct_encounters.length_of_stay_days + fct_bed_census_daily
//   door-to-provider time   → fct_appointments.wait_minutes by dim_department
const graph: ModelGraph = {
  diagrams: [],
  nodes: [
    mart("dim_patient", "Patient", "VIEW", [
      f("patient_id", "STRING", true, "Unique de-identified patient identifier."),
      f("birth_year", "INTEGER", false, "Year of birth, used for age banding."),
      f("gender", "STRING", false, "Patient gender."),
      f("postal_code", "STRING", false, "Patient postal/ZIP code for geographic analysis."),
      f("insurance_type", "STRING", false, "commercial / Medicare / Medicaid / self-pay."),
      f("risk_tier", "STRING", false, "Risk-stratification band for care management."),
      f("registered_at", "DATE", false, "Date the patient was first registered."),
    ], "One row per patient. De-identified demographics and risk stratification."),
    mart("dim_provider", "Provider", "TABLE", [
      f("provider_id", "STRING", true, "Unique provider identifier."),
      f("full_name", "STRING", false, "Provider's full name."),
      f("specialty", "STRING", false, "Clinical specialty of the provider."),
      f("department", "STRING", false, "Department the provider belongs to."),
      f("npi", "STRING", false, "National Provider Identifier."),
    ], "One row per clinician/provider."),
    mart("dim_payer", "Payer", "TABLE", [
      f("payer_id", "STRING", true, "Unique payer identifier."),
      f("name", "STRING", false, "Payer / insurance plan name."),
      f("plan_type", "STRING", false, "HMO / PPO / EPO / government."),
    ], "Reference of insurance payers / plans."),
    mart("dim_department", "Department", "TABLE", [
      f("department_id", "STRING", true, "Unique department identifier."),
      f("name", "STRING", false, "Department name."),
      f("specialty", "STRING", false, "Clinical specialty the department serves."),
      f("staffed_beds", "INTEGER", false, "Number of staffed beds — the utilization denominator."),
    ], "Reference of clinical departments with staffed-bed capacity."),
    mart("fct_appointments", "Appointments", "VIEW", [
      f("appointment_id", "STRING", true, "Unique appointment identifier."),
      f("patient_id", "STRING", false, "Patient who booked the appointment."),
      f("provider_id", "STRING", false, "Provider seeing the patient."),
      f("scheduled_at", "TIMESTAMP", false, "Scheduled date and time of the appointment."),
      f("department_id", "STRING", false, "Department where the appointment takes place."),
      f("status", "STRING", false, "Appointment status (e.g. booked, completed, cancelled)."),
      f("is_no_show", "BOOLEAN", false, "Whether the patient failed to show up."),
      f("wait_minutes", "INTEGER", false, "Door-to-provider wait."),
      f("lead_time_days", "INTEGER", false, "Booking-to-visit lead time — no-show driver."),
    ], "One row per scheduled appointment. No-show, wait time and utilization."),
    mart("fct_encounters", "Encounters", "VIEW", [
      f("encounter_id", "STRING", true, "Unique clinical encounter identifier."),
      f("appointment_id", "STRING", false, "Appointment that led to this encounter."),
      f("patient_id", "STRING", false, "Patient seen in the encounter."),
      f("provider_id", "STRING", false, "Provider who delivered care."),
      f("admit_ts", "TIMESTAMP", false, "Admission date and time."),
      f("discharge_ts", "TIMESTAMP", false, "Discharge date and time."),
      f("encounter_type", "STRING", false, "outpatient / inpatient / ED."),
      f("primary_diagnosis", "STRING", false, "Primary ICD-10 code."),
      f("length_of_stay_days", "FLOAT", false, "Length of stay in days."),
      f("is_readmission_30d", "BOOLEAN", false, "Unplanned readmission within 30 days."),
    ], "One row per clinical encounter. Diagnoses, length-of-stay and readmission."),
    mart("fct_claims", "Claims", "VIEW", [
      f("claim_id", "STRING", true, "Unique claim identifier."),
      f("encounter_id", "STRING", false, "Encounter the claim is billed for."),
      f("payer_id", "STRING", false, "Payer responsible for the claim."),
      f("submitted_at", "DATE", false, "Date the claim was submitted."),
      f("paid_at", "DATE", false, "Date the claim was paid."),
      f("billed_amount", "NUMERIC", false, "Amount billed to the payer."),
      f("allowed_amount", "NUMERIC", false, "Payer-allowed amount."),
      f("paid_amount", "NUMERIC", false, "Amount actually paid."),
      f("status", "STRING", false, "Claim status (e.g. submitted, paid, denied)."),
      f("denial_code", "STRING", false, "CARC/RARC denial reason, when denied."),
      f("ar_days", "INTEGER", false, "Days in accounts receivable — revenue-cycle speed."),
    ], "One row per claim line. Revenue cycle, denials and AR days."),
    mart("fct_bed_census_daily", "Bed Census (daily)", "VIEW", [
      f("census_id", "STRING", true, "Unique identifier for the daily census record."),
      f("department_id", "STRING", false, "Department the census covers."),
      f("census_date", "DATE", false, "Calendar day of the census."),
      f("staffed_beds", "INTEGER", false, "Beds staffed and available that day."),
      f("occupied_beds", "INTEGER", false, "Beds occupied at census time — utilization numerator."),
      f("admissions", "INTEGER", false, "Admissions during the day."),
      f("discharges", "INTEGER", false, "Discharges during the day."),
    ], "One row per department × day. Bed utilization and patient flow."),
  ],
  edges: [
    rel("e1", "fct_appointments", "dim_patient", "patient_id", "patient_id"),
    rel("e2", "fct_appointments", "dim_provider", "provider_id", "provider_id"),
    rel("e3", "fct_encounters", "fct_appointments", "appointment_id", "appointment_id"),
    rel("e4", "fct_encounters", "dim_patient", "patient_id", "patient_id"),
    rel("e5", "fct_claims", "fct_encounters", "encounter_id", "encounter_id"),
    rel("e6", "fct_claims", "dim_payer", "payer_id", "payer_id"),
    rel("e7", "fct_appointments", "dim_department", "department_id", "department_id"),
    rel("e8", "fct_bed_census_daily", "dim_department", "department_id", "department_id"),
  ],
};

export const medical: Template = {
  id: "medical",
  nicheId: "healthcare",
  category: "industry",
  name: "Healthcare",
  description: "Provider analytics: patients, providers, departments, appointments, encounters (LOS/readmission), bed census and claims/denials.",
  graph,
};
