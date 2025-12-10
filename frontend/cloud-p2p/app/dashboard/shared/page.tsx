'use client';

import { useState, useEffect } from 'react';
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card';
import { Button } from '@/components/ui/button';
import { Badge } from '@/components/ui/badge';
import { Dialog, DialogContent, DialogHeader, DialogTitle } from '@/components/ui/dialog';
import { peerApi, accessApi } from '@/lib/api/client';
import { RefreshCw, Eye, EyeOff, Loader2 } from 'lucide-react';
import { Alert, AlertDescription } from '@/components/ui/alert';
import { useAutoRefresh } from '@/lib/hooks/useAutoRefresh';

interface SharedImage {
  owner_id: string;
  image: {
    image_id: string;
    name: string;
    encrypted_path: string;
  };
  access: {
    view_count: number;
    view_limit: number;
  };
}

export default function SharedPage() {
  const [images, setImages] = useState<SharedImage[]>([]);
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [selectedImage, setSelectedImage] = useState<string | null>(null);
  const [viewingImage, setViewingImage] = useState(false);

  const loadSharedImages = async (silent = false) => {
    if (!silent) {
      setIsLoading(true);
    }
    setError(null);
    try {
      const response = await peerApi.getAccessibleImages();
      if (response.success && response.images) {
        setImages(response.images);
      } else {
        setError(response.error || 'Failed to load shared images');
      }
    } catch (err: any) {
      setError(err.message || 'Failed to load shared images');
    } finally {
      if (!silent) {
        setIsLoading(false);
      }
    }
  };

  const handleViewImage = async (ownerId: string, imageId: string) => {
    if (!confirm('View this image? This will use one of your remaining views.')) {
      return;
    }

    setViewingImage(true);
    try {
      const response = await accessApi.viewImage(ownerId, imageId);
      if (response.success && response.carrier_image_base64) {
        setSelectedImage(response.carrier_image_base64);
        loadSharedImages(); // Refresh to update view count
      } else {
        alert(response.error || 'Failed to view image');
      }
    } catch (err: any) {
      alert(err.response?.data?.error || 'Failed to view image');
    } finally {
      setViewingImage(false);
    }
  };

  useEffect(() => {
    loadSharedImages();
  }, []);

  // Auto-refresh every 5 seconds (silent mode)
  useAutoRefresh(() => loadSharedImages(true), 5000);

  const getStatusBadge = (viewCount: number, viewLimit: number) => {
    const remaining = viewLimit - viewCount;
    if (remaining <= 0) {
      return <Badge variant="destructive">Limit Reached</Badge>;
    }
    if (remaining <= 2) {
      return <Badge className="bg-yellow-500">Low Views</Badge>;
    }
    return <Badge className="bg-green-500">Active</Badge>;
  };

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-3xl font-bold text-gray-900">Shared With Me</h1>
          <p className="mt-1 text-sm text-gray-500">
            Images that others have shared with you
          </p>
        </div>
        <Button
          variant="outline"
          onClick={() => loadSharedImages()}
          disabled={isLoading}
        >
          <RefreshCw className={`h-4 w-4 ${isLoading ? 'animate-spin' : ''}`} />
        </Button>
      </div>

      {error && (
        <Alert variant="destructive">
          <AlertDescription>{error}</AlertDescription>
        </Alert>
      )}

      <Card>
        <CardHeader>
          <CardTitle>Accessible Images</CardTitle>
          <CardDescription>
            {images.length} image{images.length !== 1 ? 's' : ''} available
          </CardDescription>
        </CardHeader>
        <CardContent>
          {isLoading ? (
            <div className="flex items-center justify-center py-12">
              <Loader2 className="h-8 w-8 animate-spin text-blue-600" />
            </div>
          ) : images.length === 0 ? (
            <div className="flex flex-col items-center justify-center py-12 text-center">
              <EyeOff className="h-12 w-12 text-gray-400 mb-4" />
              <p className="text-gray-600 font-medium">No shared images</p>
              <p className="text-sm text-gray-500 mt-1">
                Request access to images from other users
              </p>
            </div>
          ) : (
            <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
              {images.map((item) => {
                const remaining = item.access.view_limit - item.access.view_count;
                const canView = remaining > 0;

                return (
                  <Card key={`${item.owner_id}-${item.image.image_id}`} className="overflow-hidden">
                    <div className={`aspect-square bg-gray-100 relative overflow-hidden ${!canView ? 'opacity-50' : ''}`}>
                      <img
                        src={`/${item.image.encrypted_path}`}
                        alt={item.image.name}
                        className="w-full h-full object-cover"
                        onError={(e) => {
                          const target = e.currentTarget;
                          target.style.display = 'none';
                          const parent = target.parentElement;
                          if (parent) {
                            parent.innerHTML = '<div class="w-full h-full flex items-center justify-center"><svg class="h-16 w-16 text-gray-300" stroke="currentColor" fill="none" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15 12a3 3 0 11-6 0 3 3 0 016 0z"></path><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M2.458 12C3.732 7.943 7.523 5 12 5c4.478 0 8.268 2.943 9.542 7-1.274 4.057-5.064 7-9.542 7-4.477 0-8.268-2.943-9.542-7z"></path></svg></div>';
                          }
                        }}
                      />
                    </div>
                    <CardContent className="p-4 space-y-3">
                      <div>
                        <p className="font-medium text-gray-900 truncate">
                          {item.image.name}
                        </p>
                        <p className="text-xs text-gray-500 mt-1">
                          From: {item.owner_id}
                        </p>
                      </div>

                      <div className="flex items-center justify-between">
                        {getStatusBadge(item.access.view_count, item.access.view_limit)}
                        <span className="text-xs text-gray-600">
                          {remaining}/{item.access.view_limit} views left
                        </span>
                      </div>

                      <Button
                        className="w-full bg-blue-600 hover:bg-blue-700"
                        onClick={() => handleViewImage(item.owner_id, item.image.image_id)}
                        disabled={!canView || viewingImage}
                      >
                        {viewingImage ? (
                          <>
                            <Loader2 className="mr-2 h-4 w-4 animate-spin" />
                            Loading...
                          </>
                        ) : canView ? (
                          <>
                            <Eye className="mr-2 h-4 w-4" />
                            View Image
                          </>
                        ) : (
                          'No Views Left'
                        )}
                      </Button>
                    </CardContent>
                  </Card>
                );
              })}
            </div>
          )}
        </CardContent>
      </Card>

      <Dialog open={!!selectedImage} onOpenChange={() => setSelectedImage(null)}>
        <DialogContent className="max-w-4xl">
          <DialogHeader>
            <DialogTitle>Image Viewer</DialogTitle>
          </DialogHeader>
          {selectedImage && (
            <div className="mt-4">
              <img
                src={`data:image/png;base64,${selectedImage}`}
                alt="Decrypted image"
                className="w-full h-auto rounded-lg"
              />
            </div>
          )}
        </DialogContent>
      </Dialog>
    </div>
  );
}
