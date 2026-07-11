import type { ModelGraph } from "@uaml/okf";
import { f, mart, rel, type Template } from "./helpers";

// HR / People Analytics — hire-to-exit model. Recruiting runs fct_requisitions
// → fct_applications (funnel, offer acceptance); the employed population lives
// in dim_employee with compensation, engagement and attrition facts around it;
// fct_headcount_monthly is the reporting rollup boards actually read.
//
// Goal coverage (niche "hr_people"):
//   time-to-fill & CPH      → fct_requisitions (time_to_fill_days, cost_per_hire)
//   offer acceptance        → fct_applications (is_offer_extended, is_offer_accepted) by source
//   first-year attrition    → fct_attrition_events (tenure_months, is_regrettable)
//   pay equity              → fct_compensation.compa_ratio × dim_position level/family
//   eNPS → attrition risk   → fct_engagement_surveys × fct_attrition_events
const graph: ModelGraph = {
  diagrams: [],
  nodes: [
    mart("dim_employee", "Employee", "VIEW", [
      f("employee_id", "STRING", true, "Unique employee identifier."),
      f("hired_at", "DATE", false, "Employment start date."),
      f("department_id", "STRING", false, "Department the employee works in."),
      f("position_id", "STRING", false, "Position the employee holds."),
      f("level", "STRING", false, "Job level or grade."),
      f("location", "STRING", false, "Work location or hub."),
      f("is_active", "BOOLEAN", false, "Whether the employee is currently employed."),
      f("terminated_at", "DATE", false, "Employment end date, if terminated."),
    ], "One row per employee, current and former."),
    mart("dim_department", "Department", "TABLE", [
      f("department_id", "STRING", true, "Unique department identifier."),
      f("name", "STRING", false, "Department name."),
      f("function", "STRING", false, "Business function (engineering, sales, G&A)."),
      f("headcount_plan", "INTEGER", false, "Planned headcount for the department."),
    ], "Reference of departments with headcount plans."),
    mart("dim_position", "Position", "TABLE", [
      f("position_id", "STRING", true, "Unique position identifier."),
      f("title", "STRING", false, "Position title."),
      f("level", "STRING", false, "Level or grade of the position."),
      f("job_family", "STRING", false, "Job family — the pay-equity comparison axis."),
      f("salary_band_min", "NUMERIC", false, "Bottom of the salary band."),
      f("salary_band_max", "NUMERIC", false, "Top of the salary band."),
    ], "Reference of positions with salary bands."),
    mart("fct_requisitions", "Requisitions", "VIEW", [
      f("requisition_id", "STRING", true, "Unique requisition identifier."),
      f("position_id", "STRING", false, "Position being hired for."),
      f("department_id", "STRING", false, "Department the hire is for."),
      f("opened_at", "DATE", false, "When the requisition opened."),
      f("filled_at", "DATE", false, "When the requisition was filled."),
      f("time_to_fill_days", "INTEGER", false, "Open-to-fill duration — time-to-fill."),
      f("cost_per_hire", "NUMERIC", false, "Total recruiting cost attributed to the hire."),
      f("top_source", "STRING", false, "Source that produced the hired candidate."),
    ], "One row per job requisition. Time-to-fill and cost-per-hire."),
    mart("fct_applications", "Applications", "VIEW", [
      f("application_id", "STRING", true, "Unique application identifier."),
      f("requisition_id", "STRING", false, "Requisition applied to."),
      f("applied_at", "DATE", false, "When the application arrived."),
      f("source", "STRING", false, "Where the candidate came from (referral, board, sourced)."),
      f("stage_reached", "STRING", false, "Furthest funnel stage: screen, interview, offer, hired."),
      f("is_offer_extended", "BOOLEAN", false, "Whether an offer was made."),
      f("is_offer_accepted", "BOOLEAN", false, "Whether the offer was accepted — acceptance rate."),
      f("rejection_reason", "STRING", false, "Why the candidate was rejected or declined."),
    ], "One row per application. The recruiting funnel and offer acceptance."),
    mart("fct_attrition_events", "Attrition Events", "VIEW", [
      f("event_id", "STRING", true, "Unique attrition-event identifier."),
      f("employee_id", "STRING", false, "Employee who left."),
      f("left_at", "DATE", false, "Last day of employment."),
      f("is_voluntary", "BOOLEAN", false, "Resignation vs termination."),
      f("is_regrettable", "BOOLEAN", false, "Whether the business wanted to keep the person."),
      f("tenure_months", "INTEGER", false, "Tenure at exit — first-year attrition reads from here."),
      f("exit_reason", "STRING", false, "Primary stated reason for leaving."),
    ], "One row per departure. Regrettable attrition by tenure and reason."),
    mart("fct_compensation", "Compensation", "VIEW", [
      f("comp_id", "STRING", true, "Unique identifier for the compensation record."),
      f("employee_id", "STRING", false, "Employee the record belongs to."),
      f("effective_date", "DATE", false, "When this compensation took effect."),
      f("base_salary", "NUMERIC", false, "Annual base salary."),
      f("bonus", "NUMERIC", false, "Target annual bonus."),
      f("equity_value", "NUMERIC", false, "Annualized equity value."),
      f("compa_ratio", "FLOAT", false, "Salary ÷ band midpoint — the pay-equity metric."),
    ], "One row per compensation change. Pay equity via compa-ratio."),
    mart("fct_engagement_surveys", "Engagement Surveys", "VIEW", [
      f("response_id", "STRING", true, "Unique survey-response identifier."),
      f("employee_id", "STRING", false, "Employee who responded."),
      f("survey_date", "DATE", false, "Survey wave date."),
      f("enps_score", "INTEGER", false, "Employee NPS answer (−100…100 scale contribution)."),
      f("engagement_score", "FLOAT", false, "Composite engagement index."),
      f("manager_score", "FLOAT", false, "Manager-effectiveness rating."),
    ], "One row per survey response. eNPS and engagement drivers."),
    mart("fct_headcount_monthly", "Headcount (monthly)", "VIEW", [
      f("snapshot_id", "STRING", true, "Unique identifier for the department-month snapshot."),
      f("department_id", "STRING", false, "Department the snapshot covers."),
      f("month", "DATE", false, "Calendar month."),
      f("headcount", "INTEGER", false, "Employees on payroll at month end."),
      f("open_positions", "INTEGER", false, "Requisitions open at month end."),
      f("attrition_rate_pct", "FLOAT", false, "Annualized attrition rate for the month."),
    ], "One row per department × month. The board-level headcount rollup."),
  ],
  edges: [
    rel("e1", "dim_employee", "dim_department", "department_id", "department_id"),
    rel("e2", "dim_employee", "dim_position", "position_id", "position_id"),
    rel("e3", "fct_requisitions", "dim_position", "position_id", "position_id"),
    rel("e4", "fct_requisitions", "dim_department", "department_id", "department_id"),
    rel("e5", "fct_applications", "fct_requisitions", "requisition_id", "requisition_id"),
    rel("e6", "fct_attrition_events", "dim_employee", "employee_id", "employee_id"),
    rel("e7", "fct_compensation", "dim_employee", "employee_id", "employee_id"),
    rel("e8", "fct_engagement_surveys", "dim_employee", "employee_id", "employee_id"),
    rel("e9", "fct_headcount_monthly", "dim_department", "department_id", "department_id"),
  ],
};

export const hr_people: Template = {
  id: "hr_people",
  nicheId: "hr_people",
  category: "industry",
  name: "HR / People Analytics",
  description: "Hire-to-exit: employees, departments, positions with bands, recruiting funnel, attrition, compensation (compa-ratio), engagement and headcount.",
  graph,
};
