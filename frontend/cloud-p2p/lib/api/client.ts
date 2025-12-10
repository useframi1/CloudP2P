import axios from 'axios';

// API Base URL - defaults to current origin
const API_BASE = typeof window !== 'undefined' ? `${window.location.origin}/api` : '/api';

// Create axios instance with default config
export const apiClient = axios.create({
  baseURL: API_BASE,
  headers: {
    'Content-Type': 'application/json',
  },
});

// Response types
export interface ApiResponse<T = any> {
  success: boolean;
  message?: string;
  error?: string;
  data?: T;
  carrier_image_base64?: string;
  client_id?: string;
  notifications?: any[];
  request_id?: string;
  peers?: any[];
  images?: any[];
  requests?: any[];
}

// Auth API
export const authApi = {
  signIn: async (clientId: string): Promise<ApiResponse> => {
    const response = await apiClient.post('/signin', { client_id: clientId });
    return response.data;
  },

  signUp: async (clientId: string): Promise<ApiResponse> => {
    const response = await apiClient.post('/signup', { client_id: clientId });
    return response.data;
  },

  signOut: async (clientId: string): Promise<ApiResponse> => {
    const response = await apiClient.post('/signout', { client_id: clientId });
    return response.data;
  },
};

// Image Management API
export const imageApi = {
  registerImages: async (): Promise<ApiResponse> => {
    const response = await apiClient.post('/register-images');
    return response.data;
  },

  uploadImage: async (imageFile: File): Promise<ApiResponse> => {
    const formData = new FormData();
    formData.append('image', imageFile);
    const response = await apiClient.post('/upload-image', formData, {
      headers: { 'Content-Type': 'multipart/form-data' },
    });
    return response.data;
  },

  getMyImages: async (): Promise<ApiResponse> => {
    const response = await apiClient.get('/my-images');
    return response.data;
  },

  encryptImage: async (imageFile: File): Promise<ApiResponse> => {
    const formData = new FormData();
    formData.append('image', imageFile);
    const response = await apiClient.post('/encrypt', formData, {
      headers: { 'Content-Type': 'multipart/form-data' },
    });
    return response.data;
  },
};

// Peer & Sharing API
export const peerApi = {
  getOnlinePeers: async (): Promise<ApiResponse> => {
    const response = await apiClient.get('/online-peers');
    return response.data;
  },

  getPeerImages: async (peerId: string): Promise<ApiResponse> => {
    const response = await apiClient.get(`/peer-images/${peerId}`);
    return response.data;
  },

  getAccessibleImages: async (): Promise<ApiResponse> => {
    const response = await apiClient.get('/accessible-images');
    return response.data;
  },

  getMySharedImages: async (): Promise<ApiResponse> => {
    const response = await apiClient.get('/my-shared-images');
    return response.data;
  },
};

// Access Control API
export const accessApi = {
  requestAccess: async (
    ownerId: string,
    imageId: string,
    reqAccessLimit?: number
  ): Promise<ApiResponse> => {
    const response = await apiClient.post('/request-access', {
      owner_id: ownerId,
      image_id: imageId,
      req_access_limit: reqAccessLimit,
    });
    return response.data;
  },

  getPendingRequests: async (): Promise<ApiResponse> => {
    const response = await apiClient.get('/pending-requests');
    return response.data;
  },

  respondToRequest: async (
    requestId: string,
    approved: boolean,
    viewLimit: number | null
  ): Promise<ApiResponse> => {
    const response = await apiClient.post('/respond-request', {
      request_id: requestId,
      approved,
      view_limit: viewLimit,
    });
    return response.data;
  },

  viewImage: async (ownerId: string, imageId: string): Promise<ApiResponse> => {
    const response = await apiClient.post('/view-image', {
      owner_id: ownerId,
      image_id: imageId,
    });
    return response.data;
  },

  manageAccess: async (
    imageId: string,
    requesterId: string,
    action: 'modify' | 'revoke',
    newViewLimit?: number
  ): Promise<ApiResponse> => {
    const response = await apiClient.post('/manage-access', {
      image_id: imageId,
      requester_id: requesterId,
      action,
      new_view_limit: newViewLimit,
    });
    return response.data;
  },

  getDefaultViewLimit: async (): Promise<ApiResponse> => {
    const response = await apiClient.get('/default-view-limit');
    return response.data;
  },
};
