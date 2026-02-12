import { useEffect } from "react";
import { search } from "../services/searchService";
import { useFileStore } from "../stores/fileStore";
import { useSearchStore } from "../stores/searchStore";

export function useSearch() {
  const query = useSearchStore((s) => s.query);
  const setResults = useSearchStore((s) => s.setResults);
  const setIsSearching = useSearchStore((s) => s.setIsSearching);
  const currentPath = useFileStore((s) => s.currentPath);

  useEffect(() => {
    if (query.length < 2) {
      setResults([]);
      return;
    }

    setIsSearching(true);
    const timer = setTimeout(() => {
      search(query, 20, currentPath || undefined)
        .then(setResults)
        .catch((err) => {
          console.error("[Search] Failed:", err);
          setResults([]);
        })
        .finally(() => setIsSearching(false));
    }, 150);

    return () => clearTimeout(timer);
  }, [query, setResults, setIsSearching, currentPath]);
}
