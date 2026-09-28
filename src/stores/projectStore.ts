import { create } from 'zustand'
import { detectStack as detectStackFromFs } from '../lib/project/detector'
import type { ProjectProfile } from '../lib/types'

interface ProjectStore {
  currentProject: ProjectProfile | null
  recentProjects: string[]
  switchProject: (path: string) => Promise<void>
  detectStack: (path: string) => Promise<ProjectProfile>
  recentPaths: string[]
}

export const useProjectStore = create<ProjectStore>((set, get) => ({
  currentProject: null,
  recentProjects: [],

  switchProject: async (path: string) => {
    const profile = await get().detectStack(path)

    set((state) => {
      const recent = state.recentProjects.includes(path)
        ? state.recentProjects
        : [path, ...state.recentProjects].slice(0, 10)
      return {
        currentProject: profile,
        recentProjects: recent,
        recentPaths: recent,
      }
    })
  },

  detectStack: async (path: string): Promise<ProjectProfile> => {
    try {
      const partial = await detectStackFromFs(path)
      return {
        id: crypto.randomUUID(),
        path,
        name: path.split('/').pop() || 'Unknown',
        stack: [],
        active_agents: [],
        active_skills: [],
        active_mcps: [],
        preferred_models: [],
        ...partial,
      }
    } catch {
      return {
        id: crypto.randomUUID(),
        path,
        name: path.split('/').pop() || 'Unknown',
        stack: [],
        active_agents: [],
        active_skills: [],
        active_mcps: [],
        preferred_models: [],
      }
    }
  },

  recentPaths: [],
}))
