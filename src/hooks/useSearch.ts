import { useEffect } from "react";
import { search } from "../services/searchService";
import { useSearchStore } from "../stores/searchStore";

export function useSearch() {
  const query = useSearchStore((s) => s.query);
  const setResults = useSearchStore((s) => s.setResults);
  const setIsSearching = useSearchStore((s) => s.setIsSearching);

  useEffect(() => {
    if (query.length < 2) {
      setResults([]);
      return;
    }

    setIsSearching(true);
    const timer = setTimeout(() => {
      search(query, 20)
        .then(setResults)
        .catch(() => setResults([]))
        .finally(() => setIsSearching(false));
    }, 150);

    return () => clearTimeout(timer);
  }, [query, setResults, setIsSearching]);
}
