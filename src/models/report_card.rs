//! Typed report card payload accepted by `POST /api/v1/report-cards`.
//!
//! The payload is validated and converted into the JSON "view model" that the
//! `templates/report_card.typ` template consumes. All arithmetic (subject
//! percentages, letter grades, grade points, the GPA, the overall result and the
//! attendance rate) happens here so the template only has to render values.

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::models::format::{DATE_FORMAT, format_date, format_percentage, format_quantity, round2};

/// School that issues the report card.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct School {
    /// Display name (required, must not be empty).
    pub name: String,
    /// Address lines, rendered one per line.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub address: Vec<String>,
    /// Contact e-mail address.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    /// Contact phone number.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub phone: Option<String>,
}

/// Guardian of the student, printed in the signature block.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Guardian {
    /// Display name (required, must not be empty).
    pub name: String,
    /// Contact e-mail address.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    /// Contact phone number.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub phone: Option<String>,
}

/// A subject the student was graded in.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportCardSubject {
    /// Subject name, e.g. `Mathematics`.
    pub name: String,
    /// Short subject code, e.g. `MATH-8`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
    /// Credit hours (weight) of the subject, defaults to `1`.
    #[serde(default = "default_credit_hours")]
    pub credit_hours: f64,
    /// Marks the student obtained.
    pub marks_obtained: f64,
    /// Maximum marks the subject is graded out of.
    pub marks_total: f64,
    /// Teacher that taught the subject.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub teacher: Option<String>,
    /// Free text remark about the subject.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remarks: Option<String>,
}

fn default_credit_hours() -> f64 {
    1.0
}

/// Attendance of the term.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Attendance {
    /// Days the student was present.
    pub present_days: u32,
    /// Days the term had in total.
    pub total_days: u32,
}

/// Full report card payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportCardPayload {
    /// Full name of the student.
    pub student_name: String,
    /// Unique student identifier, e.g. `STU-2026-0142`.
    pub student_id: String,
    /// Class the student is in, e.g. `Grade 8`.
    pub grade: String,
    /// Section of the class, e.g. `A`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub section: Option<String>,
    /// Roll number inside the section.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub roll_number: Option<String>,
    /// Academic year, e.g. `2026/2027`.
    pub academic_year: String,
    /// Term or examination the report card belongs to, e.g. `First Term`.
    pub term: String,
    /// Issue date in `YYYY-MM-DD` format.
    pub issued_date: String,
    /// Date the next term starts in `YYYY-MM-DD` format.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_term_begins: Option<String>,
    /// School that issues the report card.
    pub school: School,
    /// Guardian of the student.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub guardian: Option<Guardian>,
    /// Graded subjects (at least one is required).
    pub subjects: Vec<ReportCardSubject>,
    /// Attendance summary of the term.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attendance: Option<Attendance>,
    /// Co-curricular activities rendered as a list.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub activities: Vec<String>,
    /// Name of the class teacher.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub class_teacher: Option<String>,
    /// Remarks written by the class teacher.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub class_teacher_remarks: Option<String>,
    /// Name of the principal.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub principal: Option<String>,
    /// Position of the student in the class, if it was computed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub class_position: Option<u32>,
    /// Number of students in the class, if it is known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub class_size: Option<u32>,
}

impl ReportCardPayload {
    /// Checks the business rules of the payload.
    ///
    /// Returns every problem that was found so the caller can report them all
    /// at once instead of failing on the first issue.
    pub fn validate(&self) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();

        if self.student_name.trim().is_empty() {
            errors.push("`student_name` must not be empty".to_string());
        }

        if self.student_id.trim().is_empty() {
            errors.push("`student_id` must not be empty".to_string());
        }

        if self.grade.trim().is_empty() {
            errors.push("`grade` must not be empty".to_string());
        }

        if self.academic_year.trim().is_empty() {
            errors.push("`academic_year` must not be empty".to_string());
        }

        if self.term.trim().is_empty() {
            errors.push("`term` must not be empty".to_string());
        }

        if NaiveDate::parse_from_str(&self.issued_date, DATE_FORMAT).is_err() {
            errors.push(format!(
                "`issued_date` must be a valid date in `{DATE_FORMAT}` format, got `{}`",
                self.issued_date
            ));
        }

        if let Some(next_term) = &self.next_term_begins
            && NaiveDate::parse_from_str(next_term, DATE_FORMAT).is_err()
        {
            errors.push(format!(
                "`next_term_begins` must be a valid date in `{DATE_FORMAT}` format, got `{next_term}`"
            ));
        }

        if self.school.name.trim().is_empty() {
            errors.push("`school.name` must not be empty".to_string());
        }

        if let Some(guardian) = &self.guardian
            && guardian.name.trim().is_empty()
        {
            errors.push("`guardian.name` must not be empty".to_string());
        }

        if self.subjects.is_empty() {
            errors.push("`subjects` must contain at least one subject".to_string());
        }

        for (index, subject) in self.subjects.iter().enumerate() {
            let position = index + 1;

            if subject.name.trim().is_empty() {
                errors.push(format!(
                    "`subjects[{index}].name` (subject {position}) must not be empty"
                ));
            }

            if !subject.credit_hours.is_finite() || subject.credit_hours <= 0.0 {
                errors.push(format!(
                    "`subjects[{index}].credit_hours` (subject {position}) must be greater than 0"
                ));
            }

            if !subject.marks_total.is_finite() || subject.marks_total <= 0.0 {
                errors.push(format!(
                    "`subjects[{index}].marks_total` (subject {position}) must be greater than 0"
                ));
            } else if !subject.marks_obtained.is_finite() || subject.marks_obtained < 0.0 {
                errors.push(format!(
                    "`subjects[{index}].marks_obtained` (subject {position}) must not be negative"
                ));
            } else if subject.marks_obtained > subject.marks_total {
                errors.push(format!(
                    "`subjects[{index}].marks_obtained` (subject {position}) must not exceed `marks_total`"
                ));
            }
        }

        if let Some(attendance) = &self.attendance {
            if attendance.total_days == 0 {
                errors.push("`attendance.total_days` must be greater than 0".to_string());
            }

            if attendance.present_days > attendance.total_days {
                errors.push(
                    "`attendance.present_days` must not exceed `attendance.total_days`".to_string(),
                );
            }
        }

        if let Some(position) = self.class_position
            && position == 0
        {
            errors.push("`class_position` must be greater than 0".to_string());
        }

        if let (Some(position), Some(size)) = (self.class_position, self.class_size) {
            if size == 0 {
                errors.push("`class_size` must be greater than 0".to_string());
            } else if position > size {
                errors.push("`class_position` must not exceed `class_size`".to_string());
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    /// Validates the payload and builds the JSON view model for the template.
    pub fn to_template_data(&self) -> Result<Value, Vec<String>> {
        self.validate()?;

        let mut marks_obtained = 0.0;
        let mut marks_total = 0.0;
        let mut weighted_points = 0.0;
        let mut credit_total = 0.0;
        let mut failed_subjects = Vec::new();
        let mut subjects = Vec::with_capacity(self.subjects.len());

        for (index, subject) in self.subjects.iter().enumerate() {
            let percentage = round2(subject.marks_obtained / subject.marks_total * 100.0);
            let (grade, grade_point) = grade_for(percentage);
            let is_pass = percentage >= PASS_PERCENTAGE;

            marks_obtained += subject.marks_obtained;
            marks_total += subject.marks_total;
            weighted_points += grade_point * subject.credit_hours;
            credit_total += subject.credit_hours;

            if !is_pass {
                failed_subjects.push(subject.name.trim().to_string());
            }

            subjects.push(json!({
                "index": index + 1,
                "name": subject.name.trim(),
                "code": subject.code.clone().unwrap_or_default(),
                "credit_hours": subject.credit_hours,
                "credit_display": format_quantity(subject.credit_hours),
                "marks_obtained": subject.marks_obtained,
                "marks_obtained_display": format_quantity(subject.marks_obtained),
                "marks_total": subject.marks_total,
                "marks_total_display": format_quantity(subject.marks_total),
                "marks_display": format!(
                    "{} / {}",
                    format_quantity(subject.marks_obtained),
                    format_quantity(subject.marks_total)
                ),
                "percentage": percentage,
                "percentage_display": format_percentage(percentage),
                "grade": grade,
                "grade_point": grade_point,
                "grade_point_display": format!("{grade_point:.1}"),
                "teacher": subject.teacher.clone().unwrap_or_default(),
                "remarks": subject.remarks.clone().unwrap_or_default(),
                "is_pass": is_pass,
            }));
        }

        // The overall result weighs the subjects by their credit hours.
        let percentage = if marks_total > 0.0 {
            round2(marks_obtained / marks_total * 100.0)
        } else {
            0.0
        };
        let (overall_grade, _) = grade_for(percentage);
        let gpa = if credit_total > 0.0 {
            round2(weighted_points / credit_total)
        } else {
            0.0
        };
        let is_pass = failed_subjects.is_empty();

        let section = self.section.clone().unwrap_or_default();
        let class_label = if section.trim().is_empty() {
            self.grade.trim().to_string()
        } else {
            format!("{} ({})", self.grade.trim(), section.trim())
        };

        let activities = self
            .activities
            .iter()
            .map(|activity| activity.trim().to_string())
            .filter(|activity| !activity.is_empty())
            .collect::<Vec<_>>();

        let attendance = self
            .attendance
            .as_ref()
            .map(|attendance| {
                let rate = round2(
                    f64::from(attendance.present_days) / f64::from(attendance.total_days) * 100.0,
                );

                json!({
                    "present_days": attendance.present_days,
                    "total_days": attendance.total_days,
                    "absent_days": attendance.total_days.saturating_sub(attendance.present_days),
                    "percentage": rate,
                    "percentage_display": format_percentage(rate),
                    "is_low": rate < LOW_ATTENDANCE_PERCENTAGE,
                })
            })
            .unwrap_or_else(|| {
                json!({
                    "present_days": 0,
                    "total_days": 0,
                    "absent_days": 0,
                    "percentage": 0.0,
                    "percentage_display": String::new(),
                    "is_low": false,
                })
            });

        let rank_display = match (self.class_position, self.class_size) {
            (Some(position), Some(size)) => format!("{position} of {size}"),
            (Some(position), None) => position.to_string(),
            (None, _) => String::new(),
        };

        let guardian_name = self
            .guardian
            .as_ref()
            .map(|guardian| guardian.name.trim().to_string())
            .unwrap_or_default();
        let guardian_email = self
            .guardian
            .as_ref()
            .and_then(|guardian| guardian.email.clone())
            .unwrap_or_default();
        let guardian_phone = self
            .guardian
            .as_ref()
            .and_then(|guardian| guardian.phone.clone())
            .unwrap_or_default();

        Ok(json!({
            "school": {
                "name": self.school.name.trim(),
                "address": self.school.address,
                "email": self.school.email.clone().unwrap_or_default(),
                "phone": self.school.phone.clone().unwrap_or_default(),
            },
            "student": {
                "name": self.student_name.trim(),
                "student_id": self.student_id.trim(),
                "grade": self.grade.trim(),
                "section": section,
                "class_label": class_label,
                "roll_number": self.roll_number.clone().unwrap_or_default(),
            },
            "academic_year": self.academic_year.trim(),
            "term": self.term.trim(),
            "issued_date": {
                "raw": self.issued_date,
                "display": format_date(&self.issued_date),
            },
            "next_term": {
                "raw": self.next_term_begins.clone().unwrap_or_default(),
                "display": self
                    .next_term_begins
                    .as_deref()
                    .map(format_date)
                    .unwrap_or_default(),
            },
            "has_next_term": self.next_term_begins.is_some(),
            "guardian": {
                "name": guardian_name,
                "email": guardian_email,
                "phone": guardian_phone,
            },
            "has_guardian": self.guardian.is_some(),
            "subjects": subjects,
            "subject_count": self.subjects.len(),
            "marks_obtained": round2(marks_obtained),
            "marks_obtained_display": format_quantity(round2(marks_obtained)),
            "marks_total": round2(marks_total),
            "marks_total_display": format_quantity(round2(marks_total)),
            "percentage": percentage,
            "percentage_display": format_percentage(percentage),
            "gpa": gpa,
            "gpa_display": format!("{gpa:.2}"),
            "overall_grade": overall_grade,
            "is_pass": is_pass,
            "result_label": if is_pass { "PASS" } else { "FAIL" },
            "failed_subjects": failed_subjects,
            "has_failed_subjects": !failed_subjects.is_empty(),
            "has_rank": self.class_position.is_some(),
            "rank_display": rank_display,
            "has_attendance": self.attendance.is_some(),
            "attendance": attendance,
            "activities": activities,
            "has_activities": !activities.is_empty(),
            "class_teacher": self.class_teacher.clone().unwrap_or_default(),
            "class_teacher_remarks": self.class_teacher_remarks.clone().unwrap_or_default(),
            "has_remarks": self
                .class_teacher_remarks
                .as_deref()
                .is_some_and(|remarks| !remarks.trim().is_empty()),
            "principal": self.principal.clone().unwrap_or_default(),
            "grade_scale": grade_scale(),
        }))
    }
}

/// Minimum percentage a subject has to reach to be passed.
const PASS_PERCENTAGE: f64 = 35.0;

/// Attendance rate below which a warning is printed on the report card.
const LOW_ATTENDANCE_PERCENTAGE: f64 = 75.0;

/// Grading scale: label, lowest percentage of the band, band text and grade
/// points, best grade first.
const GRADE_SCALE: [(&str, f64, &str, f64); 8] = [
    ("A+", 90.0, "90 – 100%", 4.0),
    ("A", 80.0, "80 – 89%", 3.7),
    ("B+", 70.0, "70 – 79%", 3.3),
    ("B", 60.0, "60 – 69%", 3.0),
    ("C+", 50.0, "50 – 59%", 2.7),
    ("C", 40.0, "40 – 49%", 2.3),
    ("D", PASS_PERCENTAGE, "35 – 39%", 2.0),
    ("F", 0.0, "below 35%", 0.0),
];

/// Returns the letter grade and the grade points for a percentage.
fn grade_for(percentage: f64) -> (&'static str, f64) {
    GRADE_SCALE
        .iter()
        .find(|(_, floor, _, _)| percentage >= *floor)
        .map(|(label, _, _, points)| (*label, *points))
        .unwrap_or(("F", 0.0))
}

/// The grading scale as it is rendered as a legend on the report card.
fn grade_scale() -> Vec<Value> {
    GRADE_SCALE
        .iter()
        .map(|(label, _, range, points)| {
            json!({
                "label": label,
                "range": range,
                "points": format!("{points:.1}"),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn subject(
        name: &str,
        credit_hours: f64,
        marks_obtained: f64,
        marks_total: f64,
    ) -> ReportCardSubject {
        ReportCardSubject {
            name: name.to_string(),
            code: None,
            credit_hours,
            marks_obtained,
            marks_total,
            teacher: None,
            remarks: None,
        }
    }

    fn sample_payload() -> ReportCardPayload {
        ReportCardPayload {
            student_name: "Aarav Shrestha".to_string(),
            student_id: "STU-2026-0142".to_string(),
            grade: "Grade 8".to_string(),
            section: Some("A".to_string()),
            roll_number: Some("14".to_string()),
            academic_year: "2026/2027".to_string(),
            term: "First Term".to_string(),
            issued_date: "2026-11-08".to_string(),
            next_term_begins: Some("2027-01-12".to_string()),
            school: School {
                name: "Springfield International School".to_string(),
                address: vec!["Main Street 1".to_string()],
                email: Some("office@springfield.edu".to_string()),
                phone: None,
            },
            guardian: Some(Guardian {
                name: "Nisha Shrestha".to_string(),
                email: None,
                phone: None,
            }),
            subjects: vec![
                subject("Mathematics", 4.0, 92.0, 100.0),
                subject("English", 4.0, 84.5, 100.0),
                subject("Science", 2.0, 60.0, 100.0),
            ],
            attendance: Some(Attendance {
                present_days: 78,
                total_days: 84,
            }),
            activities: vec!["Debate club".to_string()],
            class_teacher: Some("Ms. L. Gurung".to_string()),
            class_teacher_remarks: Some("A strong term".to_string()),
            principal: Some("Dr. M. Tamang".to_string()),
            class_position: Some(3),
            class_size: Some(42),
        }
    }

    fn data_of(payload: &ReportCardPayload) -> Value {
        payload.to_template_data().expect("payload is valid")
    }

    #[test]
    fn computes_grades_totals_and_gpa() {
        let data = data_of(&sample_payload());

        // 92 + 84.5 + 60 = 236.5 out of 300.
        assert_eq!(data["marks_obtained"], json!(236.5));
        assert_eq!(data["marks_total"], json!(300.0));
        assert_eq!(data["marks_obtained_display"], json!("236.5"));
        assert_eq!(data["marks_total_display"], json!("300"));
        assert_eq!(data["percentage"], json!(78.83));
        assert_eq!(data["percentage_display"], json!("78.83%"));
        assert_eq!(data["overall_grade"], json!("B+"));
        assert_eq!(data["subject_count"], json!(3));

        // (4.0 * 4 + 3.7 * 4 + 3.0 * 2) / 10 = 3.68
        assert_eq!(data["gpa"], json!(3.68));
        assert_eq!(data["gpa_display"], json!("3.68"));

        assert_eq!(data["subjects"][0]["percentage"], json!(92.0));
        assert_eq!(data["subjects"][0]["grade"], json!("A+"));
        assert_eq!(data["subjects"][0]["grade_point_display"], json!("4.0"));
        assert_eq!(data["subjects"][1]["grade"], json!("A"));
        assert_eq!(data["subjects"][1]["marks_display"], json!("84.5 / 100"));
        assert_eq!(data["subjects"][2]["grade"], json!("B"));
    }

    #[test]
    fn derives_the_result_from_the_subjects() {
        let data = data_of(&sample_payload());
        assert_eq!(data["is_pass"], json!(true));
        assert_eq!(data["result_label"], json!("PASS"));
        assert_eq!(data["has_failed_subjects"], json!(false));
        assert_eq!(data["failed_subjects"], json!([]));

        let mut payload = sample_payload();
        payload.subjects[1].marks_obtained = 20.0;
        let data = data_of(&payload);

        assert_eq!(data["is_pass"], json!(false));
        assert_eq!(data["result_label"], json!("FAIL"));
        assert_eq!(data["has_failed_subjects"], json!(true));
        assert_eq!(data["failed_subjects"], json!(["English"]));
        assert_eq!(data["subjects"][1]["grade"], json!("F"));
        assert_eq!(data["subjects"][1]["grade_point_display"], json!("0.0"));
    }

    #[test]
    fn formats_dates_attendance_and_rank() {
        let data = data_of(&sample_payload());

        assert_eq!(data["issued_date"]["display"], json!("08 Nov 2026"));
        assert_eq!(data["next_term"]["display"], json!("12 Jan 2027"));
        assert_eq!(data["has_next_term"], json!(true));

        assert_eq!(data["attendance"]["present_days"], json!(78));
        assert_eq!(data["attendance"]["total_days"], json!(84));
        assert_eq!(data["attendance"]["absent_days"], json!(6));
        assert_eq!(data["attendance"]["percentage_display"], json!("92.86%"));
        assert_eq!(data["attendance"]["is_low"], json!(false));

        assert_eq!(data["has_rank"], json!(true));
        assert_eq!(data["rank_display"], json!("3 of 42"));
        assert_eq!(data["student"]["class_label"], json!("Grade 8 (A)"));
    }

    #[test]
    fn defaults_optional_sections() {
        let mut payload = sample_payload();
        payload.section = None;
        payload.attendance = None;
        payload.activities.clear();
        payload.guardian = None;
        payload.class_position = None;
        payload.class_size = None;
        payload.next_term_begins = None;
        let data = data_of(&payload);

        assert_eq!(data["has_attendance"], json!(false));
        assert_eq!(data["attendance"]["percentage_display"], json!(""));
        assert_eq!(data["has_activities"], json!(false));
        assert_eq!(data["has_guardian"], json!(false));
        assert_eq!(data["guardian"]["name"], json!(""));
        assert_eq!(data["has_rank"], json!(false));
        assert_eq!(data["rank_display"], json!(""));
        assert_eq!(data["has_next_term"], json!(false));
        assert_eq!(data["next_term"]["display"], json!(""));
        assert_eq!(data["student"]["class_label"], json!("Grade 8"));
        // The English mark is the only one that is not a whole number.
        assert_eq!(data["marks_obtained_display"], json!("236.5"));
    }

    #[test]
    fn flags_low_attendance() {
        let mut payload = sample_payload();
        payload.attendance = Some(Attendance {
            present_days: 40,
            total_days: 84,
        });
        let data = data_of(&payload);

        assert_eq!(data["attendance"]["is_low"], json!(true));
        assert_eq!(data["attendance"]["percentage_display"], json!("47.62%"));
    }

    #[test]
    fn maps_percentages_to_grades() {
        assert_eq!(grade_for(100.0), ("A+", 4.0));
        assert_eq!(grade_for(90.0), ("A+", 4.0));
        assert_eq!(grade_for(89.99), ("A", 3.7));
        assert_eq!(grade_for(70.0), ("B+", 3.3));
        assert_eq!(grade_for(60.0), ("B", 3.0));
        assert_eq!(grade_for(50.0), ("C+", 2.7));
        assert_eq!(grade_for(40.0), ("C", 2.3));
        assert_eq!(grade_for(35.0), ("D", 2.0));
        assert_eq!(grade_for(34.99), ("F", 0.0));
        assert_eq!(grade_for(0.0), ("F", 0.0));
    }

    #[test]
    fn rejects_invalid_payloads() {
        let mut payload = sample_payload();
        payload.student_name = "  ".to_string();
        payload.issued_date = "08/11/2026".to_string();
        payload.subjects.clear();

        let errors = payload.validate().expect_err("payload is invalid");
        assert_eq!(errors.len(), 3, "unexpected errors: {errors:?}");
    }

    #[test]
    fn rejects_bad_subjects_and_attendance() {
        let mut payload = sample_payload();
        payload.subjects[0].marks_obtained = 120.0;
        payload.subjects[1].marks_total = 0.0;
        payload.subjects[2].credit_hours = 0.0;
        payload.attendance = Some(Attendance {
            present_days: 90,
            total_days: 84,
        });
        payload.class_position = Some(50);

        let errors = payload.validate().expect_err("payload is invalid");
        assert_eq!(errors.len(), 5, "unexpected errors: {errors:?}");
    }

    #[test]
    fn renders_the_grade_scale_legend() {
        let data = data_of(&sample_payload());
        let scale = data["grade_scale"].as_array().expect("legend is an array");

        assert_eq!(scale.len(), GRADE_SCALE.len());
        assert_eq!(scale[0]["label"], json!("A+"));
        assert_eq!(scale[0]["range"], json!("90 – 100%"));
        assert_eq!(scale[0]["points"], json!("4.0"));
        assert_eq!(scale[scale.len() - 1]["label"], json!("F"));
    }
}
