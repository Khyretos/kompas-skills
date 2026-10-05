/** Fuzzy match for live filters: every word of the query must appear in the text as a
 *  subsequence (its letters in order, gaps allowed), case-insensitive; accents are ignored. */
export function fuzzyMatch(query: string, text: string): boolean {
  const nQuery = query.normalize("NFD").replace(/[\u0300-\u036f]/g, "").toLowerCase();
  const nText = text.normalize("NFD").replace(/[\u0300-\u036f]/g, "").toLowerCase();

  if (!nQuery) return true;

  const words = nQuery.split(/\s+/).filter(Boolean);

  for (const word of words) {
    let pos = 0;
    for (let i = 0; i < word.length; i++) {
      const ch = word[i];
      const idx = nText.indexOf(ch, pos);
      if (idx === -1) return false;
      pos = idx + 1;
    }
  }

  return true;
}

export function matchTask(query: string, t: { title: string; description?: string | null; state: string }, stateLabel: string): boolean {
  return fuzzyMatch(query, `${t.title} ${t.description ?? ""} ${t.state} ${stateLabel}`);
}
