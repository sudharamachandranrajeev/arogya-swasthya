// The doctor report. Everything shown comes from sys.inputs.data (JSON made
// by the app), and every string is placed as text, never run as markup.

#let d = json(bytes(sys.inputs.data))
#let ink = rgb("#1d2b27")
#let muted = rgb("#5b6b66")
#let line-c = rgb("#d5ddd9")
#let chart = rgb("#00876f")
#let band = rgb("#e3f1ea")
#let warn = rgb("#a15c00")

#set document(title: d.title, author: d.person.name)
#set page(
  paper: d.at("paper", default: "a4"),
  margin: (x: 1.8cm, top: 1.8cm, bottom: 2cm),
  footer: context [
    #set text(8pt, fill: muted)
    #d.footer #h(1fr) Page #counter(page).display() of #counter(page).final().first()
  ],
)
#set text(font: "Libertinus Serif", size: 10.5pt, fill: ink, lang: "en")
#set par(justify: false, leading: 0.55em)
#show heading.where(level: 1): set text(17pt)
#show heading.where(level: 2): it => block(above: 1.3em, below: 0.6em, sticky: true)[
  #set text(12pt)
  #it.body
  #v(-0.4em)
  #line(length: 100%, stroke: 0.6pt + line-c)
]

= #d.title

#grid(
  columns: (auto, 1fr),
  column-gutter: 1em,
  row-gutter: 0.45em,
  text(fill: muted)[Name], strong(d.person.name),
  ..if d.person.at("born", default: none) != none { (text(fill: muted)[Born], [#d.person.born]) },
  ..if d.person.at("age", default: none) != none { (text(fill: muted)[Age], [#d.person.age]) },
  ..if d.person.at("sex", default: none) != none { (text(fill: muted)[Sex], [#d.person.sex]) },
  text(fill: muted)[Period], [#d.span],
  text(fill: muted)[Made], [#d.made],
)

#if d.at("note", default: "") != "" [
  == Note from the patient
  #d.note
]

#for list in d.lists [
  == #list.heading
  #if list.rows.len() == 0 [
    #text(fill: muted)[None recorded.]
  ] else [
    #table(
      columns: (1fr, auto),
      stroke: (x, y) => (bottom: 0.4pt + line-c),
      inset: (x: 0pt, y: 5pt),
      ..list.rows.map(r => (
        [#strong(r.title)#if r.sub != "" [ \ #text(9pt, fill: muted, r.sub)]],
        align(right, text(9pt, fill: muted, r.date)),
      )).flatten()
    )
  ]
]

#if d.results.len() > 0 [
  == Test results
  #table(
    columns: (1fr, auto, auto, auto),
    column-gutter: 1.2em,
    stroke: (x, y) => (bottom: 0.4pt + line-c),
    inset: (x: 0pt, y: 5pt),
    table.header(..([Test], [Result], [Normal range], [Date]).map(c => text(9pt, fill: muted, c))),
    ..d.results.map(r => (
      [#r.name],
      if r.flag != "" { text(fill: warn)[*#r.value* (#r.flag)] } else [#r.value],
      text(fill: muted, r.range),
      text(9pt, fill: muted, r.date),
    )).flatten()
  )
]

// One series: a 2pt line (blood pressure: a bar per reading), the normal
// range shaded, y ticks on the left, dates under it.
#let plot(c) = {
  let h = 3.2cm
  block(breakable: false, width: 100%, below: 1.1em)[
    #text(weight: "bold", c.title)
    #if c.at("note", default: "") != "" [#h(0.6em)#text(9pt, fill: muted, c.note)]
    #v(0.2em)
    #grid(
      columns: (1.1cm, 1fr, 1.4cm),
      rows: (h, 0.5cm),
      // y labels
      block(width: 100%, height: h, {
        for t in c.y { place(right + top, dy: t.at(0) * 1% - 0.45em, dx: -0.3em, text(8pt, fill: muted, t.at(1))) }
      }),
      // plot
      block(width: 100%, height: h, {
        if c.band != none {
          place(top + left, dy: c.band.at(0) * 1%, rect(width: 100%, height: (c.band.at(1) - c.band.at(0)) * 1%, fill: band))
        }
        for t in c.y { place(top + left, line(start: (0%, t.at(0) * 1%), end: (100%, t.at(0) * 1%), stroke: 0.4pt + line-c)) }
        let pts = c.points
        if c.paired {
          for p in pts {
            place(top + left, line(start: (p.at(0) * 1%, p.at(1) * 1%), end: (p.at(0) * 1%, p.at(2) * 1%), stroke: 1.6pt + chart))
          }
        } else {
          for i in range(1, pts.len()) {
            let a = pts.at(i - 1)
            let b = pts.at(i)
            place(top + left, line(start: (a.at(0) * 1%, a.at(1) * 1%), end: (b.at(0) * 1%, b.at(1) * 1%), stroke: 1.6pt + chart))
          }
        }
        if pts.len() <= 60 or c.paired {
          for p in pts {
            for y in if c.paired { (p.at(1), p.at(2)) } else { (p.at(1),) } {
              place(top + left, dx: p.at(0) * 1% - 2.4pt, dy: y * 1% - 2.4pt, circle(radius: 2.4pt, fill: chart, stroke: 0.8pt + white))
            }
          }
        }
      }),
      // last value
      block(width: 100%, height: h, {
        let p = c.points.last()
        place(left + top, dy: p.at(1) * 1% - 0.45em, dx: 0.3em, text(8.5pt, weight: "bold", c.last))
      }),
      [],
      block(width: 100%, height: 0.5cm, {
        let n = c.x.len()
        for (i, t) in c.x.enumerate() {
          let label = text(8pt, fill: muted, t.at(1))
          if i == 0 and n > 1 { place(left + top, dx: t.at(0) * 1%, dy: 0.15em, label) }
          else if i == n - 1 and n > 1 { place(right + top, dx: (t.at(0) - 100) * 1%, dy: 0.15em, label) }
          else { place(center + top, dx: (t.at(0) - 50) * 1%, dy: 0.15em, label) }
        }
      }),
      [],
    )
  ]
}

#if d.charts.len() > 0 [
  == Readings over time
  #text(9pt, fill: muted)[Shaded: the lab's normal range, where one was given.]
  #v(0.4em)
  #for c in d.charts { plot(c) }
]

#if d.patterns.len() > 0 [
  == Patterns the app noticed
  #text(9pt, fill: muted)[Worked out by the app from the readings above, only for things that plausibly go together and with at least 14 days of data. They show what changed together, not what caused what.]
  #for p in d.patterns [
    #block(above: 0.8em)[*#p.title.* #p.text]
  ]
]

#v(1.2em)
#text(9pt, fill: muted)[#d.about]
