import type { ModelGraph } from "@mc/okf";
import { f, mart, rel, type Template } from "./helpers";

// EdTech / E-learning — learning-outcomes model. fct_enrollments is the
// learner-course header; fct_lesson_progress the high-volume progression
// stream (completion); fct_assessments the outcomes; fct_subscriptions carries
// monetization and early churn; fct_engagement_daily feeds retention curves.
//
// Goal coverage (niche "edtech"):
//   completion/progression  → fct_enrollments.completion_pct + fct_lesson_progress
//   free → paid             → fct_subscriptions × dim_learner.plan cohorts
//   30-day churn            → fct_subscriptions (started_at, cancelled_at) early cohort
//   assessment pass rate    → fct_assessments (is_passed, score, attempt_number)
//   engaged learning time   → fct_engagement_daily (active_mins, streak_days)
const graph: ModelGraph = {
  diagrams: [],
  nodes: [
    mart("dim_learner", "Learner", "VIEW", [
      f("learner_id", "STRING", true, "Unique learner identifier."),
      f("signup_date", "DATE", false, "When the learner registered."),
      f("acquisition_channel", "STRING", false, "Channel that brought the learner in."),
      f("country", "STRING", false, "Learner's country."),
      f("age_band", "STRING", false, "Age bucket for cohort cuts."),
      f("plan", "STRING", false, "Current plan: free or premium — conversion state."),
      f("is_active", "BOOLEAN", false, "Whether the learner was active in the last 30 days."),
    ], "One row per learner with plan and activity state."),
    mart("dim_course", "Course", "TABLE", [
      f("course_id", "STRING", true, "Unique course identifier."),
      f("title", "STRING", false, "Course title."),
      f("subject", "STRING", false, "Subject area of the course."),
      f("level", "STRING", false, "Difficulty level: beginner, intermediate, advanced."),
      f("instructor_id", "STRING", false, "Instructor who teaches the course."),
      f("lessons_count", "INTEGER", false, "Number of lessons — the completion denominator."),
      f("duration_hours", "FLOAT", false, "Total course duration in hours."),
      f("price", "NUMERIC", false, "One-off course price where sold separately."),
    ], "One row per course in the catalog."),
    mart("dim_instructor", "Instructor", "TABLE", [
      f("instructor_id", "STRING", true, "Unique instructor identifier."),
      f("name", "STRING", false, "Instructor's name."),
      f("rating", "FLOAT", false, "Average learner rating of the instructor."),
      f("courses_count", "INTEGER", false, "Courses the instructor teaches."),
    ], "Reference of instructors."),
    mart("fct_enrollments", "Enrollments", "VIEW", [
      f("enrollment_id", "STRING", true, "Unique enrollment identifier."),
      f("learner_id", "STRING", false, "Learner who enrolled."),
      f("course_id", "STRING", false, "Course enrolled in."),
      f("enrolled_at", "DATE", false, "Enrollment date."),
      f("source", "STRING", false, "How the enrollment happened (search, recommendation, bundle)."),
      f("completed_at", "DATE", false, "When the course was completed, if it was."),
      f("completion_pct", "FLOAT", false, "Share of lessons completed — course completion."),
      f("certificate_issued", "BOOLEAN", false, "Whether a certificate was earned."),
    ], "One row per learner × course. Completion is the headline metric."),
    mart("fct_lesson_progress", "Lesson Progress", "CONNECTOR", [
      f("progress_id", "STRING", true, "Unique identifier for the lesson attempt."),
      f("enrollment_id", "STRING", false, "Enrollment the progress belongs to."),
      f("lesson_number", "INTEGER", false, "Position of the lesson in the course — drop-off point."),
      f("started_at", "TIMESTAMP", false, "When the learner opened the lesson."),
      f("completed_at", "TIMESTAMP", false, "When the lesson was finished."),
      f("watch_secs", "INTEGER", false, "Seconds of video watched in the lesson."),
      f("is_completed", "BOOLEAN", false, "Whether the lesson was finished."),
    ], "One row per lesson touch — where learners stall lesson by lesson."),
    mart("fct_assessments", "Assessments", "VIEW", [
      f("attempt_id", "STRING", true, "Unique assessment-attempt identifier."),
      f("enrollment_id", "STRING", false, "Enrollment the attempt belongs to."),
      f("submitted_at", "TIMESTAMP", false, "When the attempt was submitted."),
      f("assessment_type", "STRING", false, "Kind of assessment: quiz, exam, project."),
      f("score", "FLOAT", false, "Score achieved on the attempt."),
      f("is_passed", "BOOLEAN", false, "Whether the attempt passed — the learning outcome."),
      f("attempt_number", "INTEGER", false, "Retry count for the assessment."),
    ], "One row per assessment attempt. Pass rate and retries."),
    mart("fct_subscriptions", "Subscriptions", "VIEW", [
      f("subscription_id", "STRING", true, "Unique subscription identifier."),
      f("learner_id", "STRING", false, "Learner who subscribed."),
      f("started_at", "DATE", false, "Subscription start — the day-30 churn clock starts here."),
      f("plan", "STRING", false, "Plan subscribed to."),
      f("mrr", "NUMERIC", false, "Monthly recurring revenue of the subscription."),
      f("status", "STRING", false, "Subscription status."),
      f("cancelled_at", "DATE", false, "Cancellation date, if cancelled."),
      f("churn_reason", "STRING", false, "Stated cancellation reason."),
    ], "One row per subscription. Free-to-paid conversion and early churn."),
    mart("fct_engagement_daily", "Engagement (daily)", "VIEW", [
      f("engagement_id", "STRING", true, "Unique identifier for the learner-day record."),
      f("learner_id", "STRING", false, "Learner the record covers."),
      f("activity_date", "DATE", false, "Calendar day of activity."),
      f("active_mins", "INTEGER", false, "Minutes spent learning that day."),
      f("lessons_completed", "INTEGER", false, "Lessons finished that day."),
      f("streak_days", "INTEGER", false, "Consecutive active days — the habit signal."),
    ], "One row per learner × day. Engaged learning time and streaks."),
  ],
  edges: [
    rel("e1", "dim_course", "dim_instructor", "instructor_id", "instructor_id"),
    rel("e2", "fct_enrollments", "dim_learner", "learner_id", "learner_id"),
    rel("e3", "fct_enrollments", "dim_course", "course_id", "course_id"),
    rel("e4", "fct_lesson_progress", "fct_enrollments", "enrollment_id", "enrollment_id"),
    rel("e5", "fct_assessments", "fct_enrollments", "enrollment_id", "enrollment_id"),
    rel("e6", "fct_subscriptions", "dim_learner", "learner_id", "learner_id"),
    rel("e7", "fct_engagement_daily", "dim_learner", "learner_id", "learner_id"),
  ],
};

export const edtech: Template = {
  id: "edtech",
  nicheId: "edtech",
  category: "industry",
  name: "EdTech / E-learning",
  description: "Learning outcomes: learners, courses & instructors, enrollments, lesson-level progress, assessments, subscriptions and daily engagement.",
  graph,
};
