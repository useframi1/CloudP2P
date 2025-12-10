import { create } from 'zustand';
import { persist } from 'zustand/middleware';
import { authApi } from '../api/client';

interface AuthState {
  clientId: string | null;
  isAuthenticated: boolean;
  isLoading: boolean;
  error: string | null;

  signIn: (clientId: string) => Promise<boolean>;
  signUp: (clientId: string) => Promise<boolean>;
  signOut: () => Promise<void>;
  clearError: () => void;
}

export const useAuthStore = create<AuthState>()(
  persist(
    (set, get) => ({
      clientId: null,
      isAuthenticated: false,
      isLoading: false,
      error: null,

      signIn: async (clientId: string) => {
        set({ isLoading: true, error: null });
        try {
          const response = await authApi.signIn(clientId);
          if (response.success) {
            set({
              clientId,
              isAuthenticated: true,
              isLoading: false,
              error: null
            });
            return true;
          } else {
            set({
              error: response.error || 'Sign in failed',
              isLoading: false
            });
            return false;
          }
        } catch (error: any) {
          set({
            error: error.response?.data?.error || 'Sign in failed',
            isLoading: false
          });
          return false;
        }
      },

      signUp: async (clientId: string) => {
        set({ isLoading: true, error: null });
        try {
          const response = await authApi.signUp(clientId);
          if (response.success) {
            set({
              clientId,
              isAuthenticated: true,
              isLoading: false,
              error: null
            });
            return true;
          } else {
            set({
              error: response.error || 'Sign up failed',
              isLoading: false
            });
            return false;
          }
        } catch (error: any) {
          set({
            error: error.response?.data?.error || 'Sign up failed',
            isLoading: false
          });
          return false;
        }
      },

      signOut: async () => {
        const { clientId } = get();
        if (clientId) {
          try {
            await authApi.signOut(clientId);
          } catch (error) {
            console.error('Sign out error:', error);
          }
        }
        set({
          clientId: null,
          isAuthenticated: false,
          error: null
        });
      },

      clearError: () => set({ error: null }),
    }),
    {
      name: 'auth-storage',
      partialize: (state) => ({
        clientId: state.clientId,
        isAuthenticated: state.isAuthenticated,
      }),
    }
  )
);
