// Invoice template for the PDF generator service.
//
// The service validates the payload, does all arithmetic and passes preformatted
// display strings. The template only renders; it never computes money.
//
// Input: `sys.inputs.data` — a JSON string produced by
// `InvoicePayload::to_template_data()` in `src/models/invoice.rs`.

#let raw-input = sys.inputs.at("data", default: "{}")
#let data = json(bytes(raw-input))

// ---------------------------------------------------------------- config ----

#let accent = rgb("#1f4e79")
#let muted = rgb("#6b7280")
#let border = rgb("#d7dce4")
#let surface = rgb("#f4f6f9")

#let money(value) = {
  if data.currency_symbol == "" { value } else { data.currency_symbol + value }
}

#set document(title: "Invoice " + data.invoice_number, author: data.seller.name)
#set page(
  paper: "a4",
  margin: (x: 18mm, y: 18mm),
  footer: context [
    #set text(size: 8pt, fill: muted)
    #line(length: 100%, stroke: 0.4pt + border)
    #v(2pt)
    #grid(
      columns: (1fr, 1fr),
      [#data.seller.name — Invoice #data.invoice_number],
      align(right, [Page #counter(page).display() of #context counter(page).final().first()]),
    )
  ],
)

#set text(font: "Libertinus Serif", size: 10pt, fill: rgb("#111827"))
#set par(justify: false, leading: 0.65em)

// ----------------------------------------------------------------- header ---

#grid(
  columns: (1fr, auto),
  column-gutter: 12mm,
  [
    #text(size: 18pt, weight: "bold", fill: accent)[#data.seller.name]
    #v(2pt)
    #set text(size: 9pt, fill: muted)
    #for line in data.seller.address [#line \ ]
    #if data.seller.email != "" [#link("mailto:" + data.seller.email)[#data.seller.email] \ ]
    #if data.seller.phone != "" [#data.seller.phone \ ]
    #if data.seller.tax_id != "" [Tax ID: #data.seller.tax_id]
  ],
  align(right, [
    #text(size: 24pt, weight: "bold", fill: accent)[INVOICE]
    #v(2pt)
    #text(size: 11pt)[#data.invoice_number]
    #v(1pt)
    #if data.is_paid {
      box(fill: rgb("#dcfce7"), inset: (x: 6pt, y: 2pt), radius: 3pt,
        text(fill: rgb("#166534"), weight: "bold", size: 9pt)[PAID])
    } else {
      box(fill: rgb("#fee2e2"), inset: (x: 6pt, y: 2pt), radius: 3pt,
        text(fill: rgb("#991b1b"), weight: "bold", size: 9pt)[#upper(data.status)])
    }
  ]),
)

#v(8mm)

// -------------------------------------------------------------- meta block --

#grid(
  columns: (1fr, 1fr, 1fr),
  column-gutter: 6mm,
  [
    #text(size: 8pt, fill: muted)[BILLED TO]
    #v(1pt)
    #text(weight: "bold")[#data.buyer.name]
    #v(1pt)
    #set text(size: 9pt)
    #for line in data.buyer.address [#line \ ]
    #if data.buyer.email != "" [#data.buyer.email \ ]
    #if data.buyer.phone != "" [#data.buyer.phone \ ]
    #if data.buyer.tax_id != "" [Tax ID: #data.buyer.tax_id]
  ],
  [
    #text(size: 8pt, fill: muted)[ISSUED]
    #v(1pt)
    #data.issue_date.display
    #if data.due_date != none [
      #v(4pt)
      #text(size: 8pt, fill: muted)[DUE]
      #v(1pt)
      #data.due_date.display
    ]
  ],
  [
    #text(size: 8pt, fill: muted)[TOTAL DUE]
    #v(1pt)
    #text(size: 16pt, weight: "bold", fill: accent)[#money(data.total_display)]
    #if data.po_number != "" [
      #v(4pt)
      #text(size: 8pt, fill: muted)[PO #data.po_number]
    ]
  ],
)

#v(8mm)

// ------------------------------------------------------------------ items ---

#table(
  columns: (0.6cm, 1fr, 2.2cm, 2.6cm, 2.6cm),
  inset: (x: 4pt, y: 5pt),
  align: (center, left, right, right, right),
  stroke: none,

  table.hline(stroke: 0.8pt + accent),
  text(size: 8pt, fill: muted)[\#],
  text(size: 8pt, fill: muted)[DESCRIPTION],
  text(size: 8pt, fill: muted)[QTY],
  text(size: 8pt, fill: muted)[UNIT PRICE],
  text(size: 8pt, fill: muted)[AMOUNT],
  table.hline(stroke: 0.4pt + border),

  ..data.items.map(item => (
    text(size: 9pt, fill: muted)[#item.index],
    [
      #item.description
      #if item.tax_rate > 0 [
        #text(size: 8pt, fill: muted)[ — tax #item.tax_rate%]
      ]
    ],
    [
      #item.quantity_display
      #if item.unit != "" [#text(size: 8pt, fill: muted)[ #item.unit]]
    ],
    [#money(item.unit_price_display)],
    [#money(item.amount_display)],
  )).flatten(),

  table.hline(stroke: 0.8pt + accent),
)

#v(6mm)

// ----------------------------------------------------------------- totals ---

#grid(
  columns: (1fr, 6.5cm),
  column-gutter: 8mm,
  [
    #if data.notes != "" [
      #text(size: 8pt, fill: muted)[NOTES]
      #v(2pt)
      #text(size: 9pt)[#data.notes]
    ]
    #if data.payment_terms != "" [
      #v(6pt)
      #text(size: 8pt, fill: muted)[PAYMENT TERMS]
      #v(2pt)
      #text(size: 9pt)[#data.payment_terms]
    ]
  ],
  [
    #let row(label, value, bold: false) = grid(
      columns: (1fr, auto),
      inset: (y: 2pt),
      text(size: 9pt, weight: if bold { "bold" } else { "regular" })[#label],
      align(right, text(size: 9pt, weight: if bold { "bold" } else { "regular" })[#value]),
    )

    #row([Subtotal], money(data.subtotal_display))
    #if data.has_tax [ #row([Tax], money(data.tax_total_display)) ]
    #if data.has_discount [ #row([Discount], "-" + money(data.discount_display)) ]
    #if data.has_shipping [ #row([Shipping], money(data.shipping_display)) ]
    #v(2pt)
    #line(length: 100%, stroke: 0.4pt + border)
    #v(2pt)
    #row([Total], money(data.total_display), bold: true)
  ],
)

#v(10mm)
#set text(size: 8pt, fill: muted)
Thank you for your business.
