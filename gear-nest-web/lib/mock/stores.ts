import type { Store } from "@/lib/api/types";

export const STORES: Record<string, Store> = {
  amazon: { id: "amazon", displayName: "Amazon", logoUrl: null },
  rei: { id: "rei", displayName: "REI Co-op", logoUrl: null },
  backcountry: { id: "backcountry", displayName: "Backcountry", logoUrl: null },
  cabelas: { id: "cabelas", displayName: "Cabela's", logoUrl: null },
  moosejaw: { id: "moosejaw", displayName: "Moosejaw", logoUrl: null },
  steepcheap: { id: "steepcheap", displayName: "Steep & Cheap", logoUrl: null },
  campsaver: { id: "campsaver", displayName: "CampSaver", logoUrl: null },
  ggg: { id: "ggg", displayName: "Garage Grown Gear", logoUrl: null },
};
