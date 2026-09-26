import { create } from "zustand";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  KnowledgeItem,
  KnowledgeQuery,
  KnowledgeRelation,
  SubgraphResult,
  CommunitySearchResult,
  KnowledgeContradiction,
  CodeKnowledgeJoin,
  CochangeWarning,
} from "@/lib/types";

export interface PreflightWarning {
  id: string;
  title: string;
  content: string;
  confidence: number;
  confirmation_count: number;
  stack_tags?: string;
}

export interface KnowledgeStats {
  total: number;
  avg_confidence: number;
  by_type: Array<{ type: string; count: number }>;
  by_status: Array<{ status: string; count: number }>;
  by_stack: Array<{ stack: string; count: number }>;
}

export interface KnowledgeItemInput {
  type: string;
  title: string;
  content: string;
  tags?: string;
  stack_tags?: string;
  agent_tags?: string;
  project_id?: string;
  session_ids?: string;
  plan_ids?: string;
  is_global?: boolean;
}

export interface FlywheelStats {
  sessions_processed: number;
  knowledge_items_created: number;
  confidence_avg: number;
  contradictions_found: number;
}

export interface CompounderStatus {
  last_run: string | null;
  items_since_last_run: number;
  total_items: number;
  health: 'ok' | 'stale' | 'error';
  total_runs: number;
  flywheel: FlywheelStats;
}

interface KnowledgeStore {
  items: KnowledgeItem[];
  loading: boolean;
  error: string | null;
  filters: KnowledgeQuery;
  relations: Record<string, KnowledgeRelation[]>;
  stats: KnowledgeStats | null;
  preflight: PreflightWarning[];
  preflightStack: string | null;
  compounderRunning: boolean;
  lastCompounderResult: KnowledgeItem[] | null;
  lastCompounderAt: string | null;

  compounderStatus: CompounderStatus | null;
  lastVisitTimestamp: string | null;
  newItemsSinceLastVisit: number;

  fetchCompounderStatus: (projectId?: string) => Promise<void>;
  markVisited: () => void;

  loadItems: (projectId?: string) => Promise<void>;
  loadKnowledge: (projectId?: string) => Promise<void>;
  createItem: (
    item: KnowledgeItemInput & { type: string; title: string; content: string }
  ) => Promise<KnowledgeItem>;
  addKnowledgeItem: (
    text: string,
    category: string,
    stackTags: string
  ) => Promise<KnowledgeItem>;
  updateItem: (id: string, updates: Partial<KnowledgeItem>) => Promise<void>;
  deleteItem: (id: string) => Promise<void>;
  compoundKnowledge: (projectId?: string) => Promise<void>;
  searchKnowledge: (q: string) => Promise<void>;
  loadRelations: (itemId: string) => Promise<void>;
  addRelation: (fromId: string, toId: string, type: string) => Promise<void>;
  loadStats: (projectId?: string) => Promise<void>;
  loadPreflight: (stack: string) => Promise<void>;
  runCompounder: (
    sessionId: string,
    projectId?: string
  ) => Promise<KnowledgeItem[] | null>;
  setFilters: (filters: Partial<KnowledgeQuery>) => void;
  clearError: () => void;
  // Knowledge Graph
  kgSubgraph: SubgraphResult | null;
  kgLoading: boolean;
  kgCommunities: CommunitySearchResult[];
  kgContradictions: KnowledgeContradiction[];
  kgCodeKnowledge: CodeKnowledgeJoin[];
  kgCochangeWarnings: CochangeWarning[];

  kgLocalSearch: (seedIds: string[], depth?: number) => Promise<void>;
  kgGlobalSearch: (query: string) => Promise<void>;
  kgGetCommunity: (communityId: string, level: number) => Promise<void>;
  kgGetSubgraph: (itemIds: string[], depth?: number) => Promise<void>;
  kgGetCodeKnowledge: (sourceFile: string) => Promise<void>;
  kgGetContradictions: (filter?: string) => Promise<void>;
  kgResolveContradiction: (id: string, resolution: string) => Promise<void>;
  kgMergeItems: (itemAId: string, itemBId: string) => Promise<void>;
  kgRunCommunityDetection: (projectId?: string) => Promise<void>;
  kgMineGitCochanges: (repoPath: string, projectId?: string) => Promise<void>;
  kgGetCochangeWarnings: (filePath: string, minJaccard?: number) => Promise<void>;
  subscribeKnowledgeEvents: () => Promise<UnlistenFn>;
}

export const useKnowledgeStore = create<KnowledgeStore>((set, get) => ({
  items: [],
  loading: false,
  error: null,
  filters: {},
  relations: {},
  stats: null,
  preflight: [],
  preflightStack: null,
  compounderRunning: false,
  lastCompounderResult: null,
  lastCompounderAt: null,
  compounderStatus: null,
  lastVisitTimestamp: localStorage.getItem("knowledge-last-visit"),
  newItemsSinceLastVisit: 0,
  kgSubgraph: null,
  kgLoading: false,
  kgCommunities: [],
  kgContradictions: [],
  kgCodeKnowledge: [],
  kgCochangeWarnings: [],

  loadItems: async (projectId) => {
    set({ loading: true, error: null });
    try {
      const filters = get().filters;
      const items = await invoke<KnowledgeItem[]>("get_knowledge_items_cmd", {
        q: filters.q,
        stack: filters.stack,
        agent: filters.agent,
        projectId: projectId ?? filters.project_id,
        type: filters.type,
        status: filters.status,
        minConfidence: filters.min_confidence,
        isGlobal: filters.is_global,
        limit: filters.limit,
        offset: filters.offset,
      });
      set({ items: items ?? [], loading: false });
    } catch (e) {
      set({ error: String(e), loading: false });
    }
  },

  loadKnowledge: async (projectId) => {
    await get().loadItems(projectId);
  },

  createItem: async (item) => {
    const input: KnowledgeItemInput = {
      type: item.type,
      title: item.title,
      content: item.content,
      tags: item.tags,
      stack_tags: item.stack_tags,
      agent_tags: item.agent_tags,
      project_id: item.project_id,
      session_ids: item.session_ids,
      plan_ids: item.plan_ids,
      is_global: item.is_global ?? false,
    };
    const created = await invoke<KnowledgeItem>("create_knowledge_item_cmd", {
      item: input,
    });
    set((state) => ({ items: [created, ...state.items] }));
    return created;
  },

  addKnowledgeItem: async (text, category, stackTags) => {
    const title = text.length > 60 ? text.slice(0, 57) + "..." : text;
    return get().createItem({
      type: category,
      title,
      content: text,
      stack_tags: stackTags || undefined,
    });
  },

  updateItem: async (id, updates) => {
    const payload: Record<string, unknown> = {};
    if (updates.title !== undefined) payload.title = updates.title;
    if (updates.content !== undefined) payload.content = updates.content;
    if (updates.tags !== undefined) payload.tags = updates.tags;
    if (updates.stack_tags !== undefined) payload.stack_tags = updates.stack_tags;
    if (updates.agent_tags !== undefined) payload.agent_tags = updates.agent_tags;
    if (updates.confidence !== undefined) payload.confidence = updates.confidence;
    if (updates.status !== undefined) payload.status = updates.status;

    const updated = await invoke<KnowledgeItem>("update_knowledge_item_cmd", {
      id,
      updates: payload,
    });
    set((state) => ({
      items: state.items.map((i) => (i.id === id ? updated : i)),
    }));
  },

  deleteItem: async (id) => {
    await invoke("delete_knowledge_item_cmd", { id });
    set((state) => ({
      items: state.items.filter((i) => i.id !== id),
      relations: Object.fromEntries(
        Object.entries(state.relations).filter(
          ([k]) => k !== id && !state.relations[k]?.some((r) => r.to_id === id)
        )
      ),
    }));
  },

  compoundKnowledge: async (projectId) => {
    set({ loading: true, error: null });
    try {
      const items = await invoke<KnowledgeItem[]>("compound_knowledge_cmd", {
        projectId,
      });
      set({ items: items ?? [], loading: false });
    } catch (e) {
      set({ error: String(e), loading: false });
    }
  },

  searchKnowledge: async (q) => {
    set({ loading: true, error: null });
    try {
      const items = await invoke<KnowledgeItem[]>("search_knowledge_cmd", {
        q,
      });
      const currentFilters = get().filters;
      set({ items: items ?? [], filters: { ...currentFilters, q }, loading: false });
    } catch (e) {
      set({ error: String(e), loading: false });
    }
  },

  loadRelations: async (itemId) => {
    try {
      const relations = await invoke<KnowledgeRelation[]>(
        "get_knowledge_relations_cmd",
        { fromId: itemId }
      );
      set((state) => ({
        relations: { ...state.relations, [itemId]: relations },
      }));
    } catch (e) {
      set({ error: String(e) });
    }
  },

  addRelation: async (fromId, toId, type) => {
    try {
      const rel = await invoke<KnowledgeRelation>("add_knowledge_relation_cmd", {
        fromId,
        toId,
        relationType: type,
      });
      set((state) => {
        const existing = state.relations[fromId] || [];
        return { relations: { ...state.relations, [fromId]: [rel, ...existing] } };
      });
    } catch (e) {
      set({ error: String(e) });
    }
  },

  loadStats: async (projectId) => {
    try {
      const stats = await invoke<KnowledgeStats>("get_knowledge_stats_cmd", {
        projectId,
      });
      set({ stats });
    } catch (e) {
      set({ error: String(e) });
    }
  },

  loadPreflight: async (stack) => {
    try {
      const preflight = await invoke<PreflightWarning[]>(
        "get_preflight_warnings_cmd",
        { stack }
      );
      set({ preflight, preflightStack: stack });
    } catch (e) {
      set({ error: String(e), preflight: [], preflightStack: stack });
    }
  },

  runCompounder: async (sessionId, projectId) => {
    set({ compounderRunning: true, error: null });
    try {
      const items = await invoke<KnowledgeItem[] | null>("run_compounder_cmd", {
        sessionId,
        projectId,
      });
      const result = items || [];
      set({
        lastCompounderResult: result,
        lastCompounderAt: new Date().toISOString(),
        compounderRunning: false,
      });
      if (result.length > 0) {
        await get().loadItems(projectId);
      }
      // Refresh compounder status after run
      await get().fetchCompounderStatus(projectId);
      return result;
    } catch (e) {
      set({ error: String(e), compounderRunning: false });
      return null;
    }
  },

  fetchCompounderStatus: async (projectId) => {
    try {
      const status = await invoke<CompounderStatus>("get_compounder_status_cmd", {
        projectId,
      });
      set({ compounderStatus: status });
      if (!status) return;
      
      // Calculate new items since last visit
      const lastVisit = get().lastVisitTimestamp;
      if (lastVisit && status.last_run) {
        // If compounder ran after last visit, count items
        const lastVisitDate = new Date(lastVisit);
        const lastRunDate = new Date(status.last_run);
        if (lastRunDate > lastVisitDate) {
          set({ newItemsSinceLastVisit: status.items_since_last_run });
        }
      } else if (!lastVisit && status.last_run) {
        // First visit, no new items to badge
        set({ newItemsSinceLastVisit: 0 });
      }
    } catch (e) {
      console.error("Failed to fetch compounder status:", e);
    }
  },

  markVisited: () => {
    const now = new Date().toISOString();
    localStorage.setItem("knowledge-last-visit", now);
    set({ lastVisitTimestamp: now, newItemsSinceLastVisit: 0 });
  },

  setFilters: (filters) => {
    set((state) => ({ filters: { ...state.filters, ...filters } }));
  },

  clearError: () => set({ error: null }),

  // Knowledge Graph actions
  kgLocalSearch: async (seedIds, depth) => {
    set({ kgLoading: true, error: null });
    try {
      const result = await invoke<SubgraphResult>("kg_local_search_cmd", {
        seedIds,
        depth: depth ?? 2,
      });
      set({ kgSubgraph: result, kgLoading: false });
    } catch (e) {
      set({ error: String(e), kgLoading: false });
    }
  },

  kgGlobalSearch: async (query) => {
    set({ kgLoading: true, error: null });
    try {
      const communities = await invoke<CommunitySearchResult[]>("kg_global_search_cmd", {
        query,
        limit: 10,
      });
      set({ kgCommunities: communities, kgLoading: false });
    } catch (e) {
      set({ error: String(e), kgLoading: false });
    }
  },

  kgGetCommunity: async (communityId, level) => {
    set({ kgLoading: true, error: null });
    try {
      await invoke("kg_get_community_cmd", { communityId, level });
      set({ kgLoading: false });
    } catch (e) {
      set({ error: String(e), kgLoading: false });
    }
  },

  kgGetSubgraph: async (itemIds, depth) => {
    set({ kgLoading: true, error: null });
    try {
      const result = await invoke<SubgraphResult>("kg_get_subgraph_cmd", {
        itemIds,
        depth: depth ?? 2,
      });
      set({ kgSubgraph: result, kgLoading: false });
    } catch (e) {
      set({ error: String(e), kgLoading: false });
    }
  },

  kgGetCodeKnowledge: async (sourceFile) => {
    set({ kgLoading: true, error: null });
    try {
      const links = await invoke<CodeKnowledgeJoin[]>("kg_get_code_knowledge_cmd", {
        sourceFile,
      });
      set({ kgCodeKnowledge: links, kgLoading: false });
    } catch (e) {
      set({ error: String(e), kgLoading: false });
    }
  },

  kgGetContradictions: async (filter) => {
    try {
      const contradictions = await invoke<KnowledgeContradiction[]>("kg_get_contradictions_cmd", {
        filter: filter ?? null,
      });
      set({ kgContradictions: contradictions });
    } catch (e) {
      set({ error: String(e) });
    }
  },

  kgResolveContradiction: async (id, resolution) => {
    try {
      await invoke("kg_resolve_contradiction_cmd", {
        id,
        resolution,
        resolvedBy: "user",
      });
      await get().kgGetContradictions();
    } catch (e) {
      set({ error: String(e) });
    }
  },

  kgMergeItems: async (itemAId, itemBId) => {
    set({ loading: true, error: null });
    try {
      await invoke("kg_merge_items_cmd", { itemAId, itemBId });
      await get().loadItems();
    } catch (e) {
      set({ error: String(e), loading: false });
    }
  },

  kgRunCommunityDetection: async (projectId) => {
    set({ kgLoading: true, error: null });
    try {
      await invoke("kg_run_community_detection_cmd", { projectId });
      set({ kgLoading: false });
    } catch (e) {
      set({ error: String(e), kgLoading: false });
    }
  },

  kgMineGitCochanges: async (repoPath, projectId) => {
    set({ kgLoading: true, error: null });
    try {
      await invoke("kg_mine_git_cochanges_cmd", { repoPath, projectId });
      set({ kgLoading: false });
    } catch (e) {
      set({ error: String(e), kgLoading: false });
    }
  },

  kgGetCochangeWarnings: async (filePath, minJaccard) => {
    set({ kgLoading: true, error: null });
    try {
      const warnings = await invoke<CochangeWarning[]>("kg_get_cochange_warnings_cmd", {
        filePath,
        minJaccard: minJaccard ?? 0.3,
      });
      set({ kgCochangeWarnings: warnings, kgLoading: false });
    } catch (e) {
      set({ error: String(e), kgLoading: false });
    }
  },

  subscribeKnowledgeEvents: async () => {
    const unlisten = await listen<KnowledgeItem>(
      "knowledge-item-created",
      (event) => {
        const item = event.payload;
        set((state) => {
          if (state.items.some((i) => i.id === item.id)) return state;
          return { items: [item, ...state.items] };
        });
      }
    );
    return unlisten;
  },
}));
