import { useEffect, useState } from "react";
import { format } from "date-fns";

function currentLocalDay() {
  return format(new Date(), "yyyy-MM-dd");
}

export function useLocalDay() {
  const [day, setDay] = useState(currentLocalDay);

  useEffect(() => {
    const update = () => setDay((current) => {
      const next = currentLocalDay();
      return current === next ? current : next;
    });
    const interval = window.setInterval(update, 30_000);
    document.addEventListener("visibilitychange", update);
    return () => {
      window.clearInterval(interval);
      document.removeEventListener("visibilitychange", update);
    };
  }, []);

  return day;
}
