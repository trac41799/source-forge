import { useEffect } from "react";
import { Link, useLocation } from "react-router-dom";
import {
  Rocket, Map, Workflow, FolderOpen, BarChart3, Clock,
  Boxes, BookMarked, Brain, Clock4, Settings, DollarSign,
  Hammer, Moon, Sun,
} from "lucide-react";
import { cn } from "@/lib/utils";
import { Button } from "@/components/ui/button";
import { Separator } from "@/components/ui/separator";
import { ScrollArea } from "@/components/ui/scroll-area";
import {
  Tooltip, TooltipContent, TooltipProvider, TooltipTrigger,
} from "@/components/ui/tooltip";
import { useTheme } from "@/components/ThemeProvider";
import { useKnowledgeStore } from "@/stores/knowledgeStore";
import { useSettingsStore } from "@/stores/settingsStore";
import { CollapsibleGroup } from "@/components/layout/CollapsibleGroup";

interface NavItem {
  path: string;
  label: string;
  icon: typeof Rocket;
  shortcut?: string;
}

interface NavGroup {
  id: string;
  label: string;
  items: NavItem[];
}

const navGroups: NavGroup[] = [
  {
    id: "work",
    label: "WORK",
    items: [
      { path: "/build", label: "Build App", icon: Hammer },
      { path: "/orchestrate", label: "Orchestrate", icon: Workflow },
      { path: "/knowledge", label: "Knowledge", icon: Brain },
    ],
  },
  {
    id: "review",
    label: "REVIEW",
    items: [
      { path: "/outcomes", label: "Outcomes", icon: BarChart3 },
      { path: "/replay", label: "Replay", icon: Clock },
    ],
  },
  {
    id: "configure",
    label: "CONFIGURE",
    items: [
      { path: "/route", label: "Route", icon: Map },
      { path: "/assets", label: "Assets", icon: FolderOpen },
      { path: "/integrations", label: "Integrations", icon: BookMarked },
    ],
  },
  {
    id: "automate",
    label: "AUTOMATE",
    items: [
      { path: "/scheduler", label: "Scheduler", icon: Clock4 },
      { path: "/playbooks", label: "Playbooks", icon: Boxes },
    ],
  },
  {
    id: "system",
    label: "SYSTEM",
    items: [
      { path: "/costs", label: "Costs", icon: DollarSign },
      { path: "/settings", label: "Settings", icon: Settings },
    ],
  },
];

export function Sidebar() {
  const location = useLocation();
  const { theme, setTheme } = useTheme();
  const { sidebarCollapsed, toggleSidebarGroup } = useSettingsStore();
  const newItems = useKnowledgeStore((s) => s.newItemsSinceLastVisit);

  useEffect(() => {
    const activeGroup = navGroups.find((g) =>
      g.items.some((i) => location.pathname.startsWith(i.path))
    );
    if (activeGroup && sidebarCollapsed[activeGroup.id]) {
      toggleSidebarGroup(activeGroup.id);
    }
  }, [location.pathname]);

  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (e.ctrlKey || e.metaKey) {
        const shortcuts: Record<string, string> = {
          "1": "/runner",
          "2": "/orchestrate",
          "3": "/knowledge",
          "4": "/outcomes",
          "5": "/replay",
          ",": "/settings",
        };
        const target = shortcuts[e.key];
        if (target) {
          e.preventDefault();
          window.location.href = target;
        }
      }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, []);

  const runnerActive = location.pathname === "/runner";

  return (
    <TooltipProvider delayDuration={0}>
      <div className="flex h-full w-[240px] flex-col glass-panel relative">
        <div className="absolute right-0 top-0 bottom-0 w-px gradient-sidebar-accent" />

        <div className="flex h-14 items-center gap-3 px-4">
          <div className="size-8 rounded-lg gradient-primary flex items-center justify-center shadow-glow">
            <span className="text-white text-sm font-bold">A</span>
          </div>
          <span className="text-base font-bold text-foreground tracking-tight">
            SourceForge
          </span>
        </div>

        <Separator className="border-glass-border" />

        <div className="p-2">
          <Tooltip>
            <TooltipTrigger asChild>
              <Button
                variant="ghost"
                className={cn(
                  "justify-start gap-2.5 px-3 font-normal w-full",
                  runnerActive
                    ? "bg-indigo-500/10 text-indigo-300 border border-indigo-500/15"
                    : "text-muted-foreground"
                )}
                asChild
              >
                <Link to="/runner">
                  <Rocket className="size-4" />
                  <span>Runner</span>
                  <span className="ml-auto text-[10px] text-muted-foreground/40">Ctrl+1</span>
                </Link>
              </Button>
            </TooltipTrigger>
            <TooltipContent side="right">Runner</TooltipContent>
          </Tooltip>
        </div>
        <Separator className="border-glass-border" />

        <ScrollArea className="flex-1">
          <div className="flex flex-col p-2">
            {navGroups.map((group) => {
              const isCollapsed = sidebarCollapsed[group.id] ?? true;
              const groupActive = group.items.some((i) =>
                location.pathname.startsWith(i.path)
              );
              const badgeCount = group.id === "work" ? newItems : 0;

              const handleToggle = () => {
                // Accordion: close all other groups when expanding one
                if (isCollapsed) {
                  const others = Object.keys(sidebarCollapsed).filter(
                    (id) => id !== group.id && !sidebarCollapsed[id]
                  );
                  others.forEach((id) => {
                    if (!sidebarCollapsed[id]) {
                      toggleSidebarGroup(id);
                    }
                  });
                }
                toggleSidebarGroup(group.id);
              };

              return (
                <CollapsibleGroup
                  key={group.id}
                  label={group.label}
                  isCollapsed={isCollapsed}
                  isActive={groupActive}
                  badgeCount={isCollapsed ? badgeCount : 0}
                  onToggle={handleToggle}
                >
                  {group.items.map((item) => {
                    const Icon = item.icon;
                    const isActive = location.pathname === item.path;

                    return (
                      <Tooltip key={item.path}>
                        <TooltipTrigger asChild>
                          <Button
                            variant="ghost"
                            className={cn(
                              "justify-start gap-2.5 px-3 font-normal w-full",
                              isActive
                                ? "bg-indigo-500/10 text-indigo-300 border border-indigo-500/15"
                                : "text-muted-foreground"
                            )}
                            asChild
                          >
                            <Link to={item.path}>
                              <Icon className="size-4" />
                              <span>{item.label}</span>
                            </Link>
                          </Button>
                        </TooltipTrigger>
                        <TooltipContent side="right">{item.label}</TooltipContent>
                      </Tooltip>
                    );
                  })}
                </CollapsibleGroup>
              );
            })}
          </div>
        </ScrollArea>

        <div className="p-3 border-t border-glass-border">
          <div className="flex bg-glass-20 rounded-lg p-0.5 border border-glass-border">
            <button
              onClick={() => setTheme("dark")}
              className={cn(
                "flex-1 flex items-center justify-center gap-1.5 py-1.5 rounded-md text-xs font-medium transition-all duration-200",
                theme === "dark"
                  ? "gradient-primary text-white shadow-glow-sm"
                  : "text-muted-foreground hover:text-foreground"
              )}
            >
              <Moon className="size-3" />
              Dark
            </button>
            <button
              onClick={() => setTheme("light")}
              className={cn(
                "flex-1 flex items-center justify-center gap-1.5 py-1.5 rounded-md text-xs font-medium transition-all duration-200",
                theme === "light"
                  ? "gradient-primary text-white shadow-glow-sm"
                  : "text-muted-foreground hover:text-foreground"
              )}
            >
              <Sun className="size-3" />
              Light
            </button>
          </div>
        </div>
      </div>
    </TooltipProvider>
  );
}
