// Deterministic 1000/3000-page PDF fixture for the overlay baseline. Kept in plain JS so the
// benchmark has no Python/PDF dependency and can run on CI or a fresh checkout.
//
// The pages are intentionally simple text pages: this measures reader chrome (page shells, range
// overlay and pointer path), not PDF.js canvas throughput.

function contentFor(number, pageCount) {
  return [
    "BT",
    "/F1 24 Tf",
    "72 720 Td",
    `(PDFScope overlay fixture) Tj`,
    "0 -36 Td",
    "/F1 14 Tf",
    `(Page ${number} of ${pageCount}) Tj`,
    "0 -30 Td",
    "/F1 11 Tf",
    "(Same shape on every page so page geometry is predictable.) Tj",
    "ET",
  ].join("\n");
}

export function makePerfPdf(pageCount) {
  const objects = [];
  const add = (body) => {
    objects.push(body);
    return objects.length;
  };

  add("<< /Type /Catalog /Pages 2 0 R >>");
  add("");
  add("<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>");

  const kids = [];
  for (let number = 1; number <= pageCount; number += 1) {
    const content = contentFor(number, pageCount);
    const contentObject = add(
      `<< /Length ${Buffer.byteLength(content)} >>\nstream\n${content}\nendstream`,
    );
    const pageObject = add(
      `<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Resources << /Font << /F1 3 0 R >> >> /Contents ${contentObject} 0 R >>`,
    );
    kids.push(`${pageObject} 0 R`);
  }
  objects[1] = `<< /Type /Pages /Kids [${kids.join(" ")}] /Count ${pageCount} >>`;

  let pdf = "%PDF-1.7\n%\xE2\xE3\xCF\xD3\n";
  const offsets = [0];
  objects.forEach((body, index) => {
    offsets[index + 1] = Buffer.byteLength(pdf, "latin1");
    pdf += `${index + 1} 0 obj\n${body}\nendobj\n`;
  });

  const xref = Buffer.byteLength(pdf, "latin1");
  pdf += `xref\n0 ${objects.length + 1}\n`;
  pdf += "0000000000 65535 f \n";
  for (let index = 1; index <= objects.length; index += 1) {
    pdf += `${String(offsets[index]).padStart(10, "0")} 00000 n \n`;
  }
  pdf += `trailer\n<< /Size ${objects.length + 1} /Root 1 0 R >>\nstartxref\n${xref}\n%%EOF\n`;
  return Buffer.from(pdf, "latin1");
}
