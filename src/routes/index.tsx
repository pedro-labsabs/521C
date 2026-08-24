import { createFileRoute } from "@tanstack/react-router";
import { HubShell } from "@/components/hub/shell";

export const Route = createFileRoute("/")({ component: Home });

function Home() {
  return <HubShell />;
}
