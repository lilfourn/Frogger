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
    let active = true;

    if (query.length < 2) {
      setResults([]);
      setIsSearching(false);
      return;
    }

    setIsSearching(true);
    const timer = setTimeout(() => {
      search(query, 20, currentPath || undefined)
        .then((results) => {
          if (!active) return;
          setResults(results);
        })
        .catch((err) => {
          if (!active) return;
          console.error("[Search] Failed:", err);
          setResults([]);
        })
        .finally(() => {
          if (!active) return;
          setIsSearching(false);
        });
    }, 150);

    return () => {
      active = false;
      clearTimeout(timer);
    };
  }, [query, setResults, setIsSearching, currentPath]);
}
