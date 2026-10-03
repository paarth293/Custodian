export interface SearchHit {
  id: string;
  label: string;
}

/** Local file search. Placeholder: returns no results until the index lands. */
export function search(query: string): SearchHit[] {
  void query;
  return [];
}
