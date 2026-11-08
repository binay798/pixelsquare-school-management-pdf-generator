// Student report card.
//
// Rendered from the view model that `src/models/report_card.rs` builds, so every
// number, date and grade arrives as a ready-to-print string. The template only
// lays the values out.

#let raw-input = sys.inputs.at("data", default: "{}")
#let data = json(bytes(raw-input))

#let accent = rgb("#1f4e79")
#let muted = rgb("#6b7280")
#let border = rgb("#d7dce4")
#let surface = rgb("#f4f6f9")
#let pass-color = rgb("#166534")
#let pass-surface = rgb("#dcfce7")
#let fail-color = rgb("#991b1b")
#let fail-surface = rgb("#fee2e2")

#set document(
  title: "Report card " + data.student.name,
  author: data.school.name,
)

#set page(
  paper: "a4",
  margin: (x: 16mm, y: 12mm),
  footer: context [
    #set text(size: 8pt, fill: muted)
    #line(length: 100%, stroke: 0.4pt + border)
    #v(2pt)
    #grid(
      columns: (1fr, 1fr),
      [#data.school.name — Report card #data.student.student_id],
      align(right, [
        Page #counter(page).display() of #context counter(page).final().first()
      ]),
    )
  ],
)

#set text(font: "Libertinus Serif", size: 10pt, fill: rgb("#111827"))
#set par(justify: false, leading: 0.65em)

// ── Header ────────────────────────────────────────────────────────────────
#grid(
  columns: (1fr, auto),
  column-gutter: 10mm,
  [
    #text(size: 15pt, weight: "bold", fill: accent)[#data.school.name]
    #v(3pt)
    #set text(size: 8.5pt, fill: muted)
    #for line in data.school.address [#line \ ]
    #if data.school.phone != "" [#data.school.phone \ ]
    #if data.school.email != "" [#data.school.email]
  ],
  align(right, [
    #text(size: 21pt, weight: "bold", fill: accent)[REPORT CARD]
    #v(3pt)
    #text(size: 10pt)[#data.term]
    #v(1pt)
    #text(size: 9pt, fill: muted)[Academic year #data.academic_year]
  ]),
)

#v(3mm)
#line(length: 100%, stroke: 0.8pt + accent)
#v(4mm)

// ── Student ───────────────────────────────────────────────────────────────
#block(
  width: 100%,
  fill: surface,
  inset: 8pt,
  radius: 4pt,
  grid(
    columns: (1fr, 1fr, 1fr, 1fr),
    column-gutter: 6mm,
    row-gutter: 3mm,
    [
      #text(size: 7.5pt, fill: muted)[STUDENT]
      #linebreak()
      #text(size: 10.5pt, weight: "bold")[#data.student.name]
    ],
    [
      #text(size: 7.5pt, fill: muted)[STUDENT ID]
      #linebreak()
      #text(size: 9.5pt)[#data.student.student_id]
    ],
    [
      #text(size: 7.5pt, fill: muted)[CLASS]
      #linebreak()
      #text(size: 9.5pt)[#data.student.class_label]
    ],
    [
      #text(size: 7.5pt, fill: muted)[ROLL NO.]
      #linebreak()
      #text(size: 9.5pt)[#if data.student.roll_number != "" [#data.student.roll_number] else [—]]
    ],
    [
      #text(size: 7.5pt, fill: muted)[GUARDIAN]
      #linebreak()
      #text(size: 9.5pt)[#if data.has_guardian [#data.guardian.name] else [—]]
    ],
    [
      #text(size: 7.5pt, fill: muted)[ISSUED ON]
      #linebreak()
      #text(size: 9.5pt)[#data.issued_date.display]
    ],
    [
      #text(size: 7.5pt, fill: muted)[SUBJECTS]
      #linebreak()
      #text(size: 9.5pt)[#data.subject_count]
    ],
    [
      #text(size: 7.5pt, fill: muted)[ATTENDANCE]
      #linebreak()
      #text(size: 9.5pt)[#if data.has_attendance [#data.attendance.percentage_display] else [—]]
    ],
  ),
)

// ── Subjects ──────────────────────────────────────────────────────────────
#v(3mm)
#text(size: 8pt, fill: muted)[SUBJECT RESULTS]
#v(2pt)

#table(
  columns: (0.7cm, 1fr, 1.2cm, 2.1cm, 1.5cm, 1.2cm, 1.2cm),
  inset: (x: 4pt, y: 3pt),
  align: (center, left, center, right, right, center, center),
  stroke: none,
  table.hline(stroke: 0.8pt + accent),
  text(size: 7.5pt, fill: muted)[\#],
  text(size: 7.5pt, fill: muted)[SUBJECT],
  text(size: 7.5pt, fill: muted)[CREDIT],
  text(size: 7.5pt, fill: muted)[MARKS],
  text(size: 7.5pt, fill: muted)[PERCENT],
  text(size: 7.5pt, fill: muted)[GRADE],
  text(size: 7.5pt, fill: muted)[POINTS],
  table.hline(stroke: 0.4pt + border),

  ..data.subjects
    .map(subject => (
      text(size: 9pt, fill: muted)[#subject.index],
      [
        #text(size: 9.5pt, weight: "medium")[#subject.name]
        #if subject.code != "" [
          #text(size: 7.5pt, fill: muted)[ · #subject.code]
        ]
        #if subject.teacher != "" [
          #linebreak()
          #text(size: 7.5pt, fill: muted)[#subject.teacher]
        ]
        #if subject.remarks != "" [
          #linebreak()
          #text(size: 7.5pt, style: "italic", fill: muted)[#subject.remarks]
        ]
      ],
      text(size: 9pt)[#subject.credit_display],
      text(size: 9pt)[#subject.marks_display],
      text(size: 9pt)[#subject.percentage_display],
      text(
        size: 9.5pt,
        weight: "bold",
        fill: if subject.is_pass { pass-color } else { fail-color },
      )[#subject.grade],
      text(size: 9pt)[#subject.grade_point_display],
    ))
    .flatten(),

  table.hline(stroke: 0.8pt + accent),
)

// ── Result ────────────────────────────────────────────────────────────────
#v(4mm)
#grid(
  columns: (1fr, 7cm),
  column-gutter: 8mm,
  [
    #if data.has_attendance [
      #text(size: 8pt, fill: muted)[ATTENDANCE]
      #v(2pt)
      #text(size: 9.5pt)[#data.attendance.present_days of #data.attendance.total_days days present (#data.attendance.percentage_display)]
      #v(1pt)
      #text(size: 8.5pt, fill: muted)[#data.attendance.absent_days days absent]
      #if data.attendance.is_low [
        #v(2pt)
        #text(size: 8.5pt, fill: fail-color)[Attendance below the 75% requirement]
      ]
      #v(5pt)
    ]
    #if data.has_activities [
      #text(size: 8pt, fill: muted)[CO-CURRICULAR ACTIVITIES]
      #v(2pt)
      #set text(size: 9pt)
      #for activity in data.activities [- #activity \ ]
    ]
    #if data.has_next_term [
      #v(5pt)
      #text(size: 8pt, fill: muted)[NEXT TERM BEGINS]
      #v(2pt)
      #text(size: 9pt)[#data.next_term.display]
    ]
  ],
  [
    #let row(label, value) = grid(
      columns: (1fr, auto),
      inset: (y: 2.5pt),
      text(size: 9pt, fill: muted)[#label],
      align(right, text(size: 9.5pt, weight: "bold")[#value]),
    )

    #row([Marks obtained], data.marks_obtained_display + " / " + data.marks_total_display)
    #row([Percentage], data.percentage_display)
    #row([Grade], data.overall_grade)
    #row([GPA], data.gpa_display)
    #if data.has_rank [#row([Class position], data.rank_display)]
    #v(4pt)
    #grid(
      columns: (auto, 1fr),
      column-gutter: 6pt,
      align: horizon,
      box(
        fill: if data.is_pass { pass-surface } else { fail-surface },
        inset: (x: 8pt, y: 3pt),
        radius: 3pt,
        text(
          size: 9pt,
          weight: "bold",
          fill: if data.is_pass { pass-color } else { fail-color },
        )[#data.result_label],
      ),
      if data.has_failed_subjects [
        #text(size: 8pt, fill: fail-color)[Not passed: #(data.failed_subjects.join(", "))]
      ],
    )
  ],
)

// ── Remarks and signatures ────────────────────────────────────────────────
#if data.has_remarks [
  #v(4mm)
  #text(size: 8pt, fill: muted)[CLASS TEACHER'S REMARKS]
  #v(2pt)
  #block(
    width: 100%,
    fill: surface,
    inset: 7pt,
    radius: 4pt,
    text(size: 9.5pt)[#data.class_teacher_remarks],
  )
]

#v(6mm)
#block(breakable: false)[
  #grid(
    columns: (1fr, 1fr, 1fr),
    column-gutter: 10mm,
    [
      #line(length: 100%, stroke: 0.5pt + border)
      #v(2pt)
      #text(size: 9pt, weight: "bold")[#data.class_teacher]
      #linebreak()
      #text(size: 8pt, fill: muted)[Class teacher]
    ],
    [
      #line(length: 100%, stroke: 0.5pt + border)
      #v(2pt)
      #text(size: 9pt, weight: "bold")[#data.guardian.name]
      #linebreak()
      #text(size: 8pt, fill: muted)[Guardian]
    ],
    [
      #line(length: 100%, stroke: 0.5pt + border)
      #v(2pt)
      #text(size: 9pt, weight: "bold")[#data.principal]
      #linebreak()
      #text(size: 8pt, fill: muted)[Principal]
    ],
  )
  #v(5mm)
  #text(size: 7.5pt, fill: muted)[GRADING SCALE]
  #v(1pt)
  #text(size: 7pt, fill: muted)[
    #(
      data.grade_scale
        .map(entry => entry.label + " = " + entry.points + " (" + entry.range + ")")
        .join("  ·  ")
    )
  ]
]
