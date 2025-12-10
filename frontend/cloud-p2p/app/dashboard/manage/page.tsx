'use client';

import { useState, useEffect } from 'react';
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { peerApi, accessApi } from '@/lib/api/client';
import { RefreshCw, Edit2, Trash2, Loader2, ImageOff } from 'lucide-react';
import { Alert, AlertDescription } from '@/components/ui/alert';

interface AccessEntry {
  requester_id: string;
  view_limit: number;
  view_count: number;
  views_left: number;
}

interface SharedImage {
  image_id: string;
  name: string;
  total_shared_with: number;
  access_list: AccessEntry[];
}

export default function ManagePage() {
  const [images, setImages] = useState<SharedImage[]>([]);
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [editingLimits, setEditingLimits] = useState<Record<string, number>>({});

  const loadSharedImages = async () => {
    setIsLoading(true);
    setError(null);
    try {
      const response = await peerApi.getMySharedImages();
      if (response.success && response.images) {
        setImages(response.images);
        // Initialize editing limits
        const limits: Record<string, number> = {};
        response.images.forEach((img: SharedImage) => {
          img.access_list.forEach((access) => {
            limits[`${img.image_id}-${access.requester_id}`] = access.view_limit;
          });
        });
        setEditingLimits(limits);
      } else {
        setError(response.error || 'Failed to load shared images');
      }
    } catch (err: any) {
      setError(err.message || 'Failed to load shared images');
    } finally {
      setIsLoading(false);
    }
  };

  const handleModifyAccess = async (imageId: string, requesterId: string) => {
    const key = `${imageId}-${requesterId}`;
    const newLimit = editingLimits[key];

    if (!newLimit || newLimit < 1) {
      alert('Please enter a valid view limit (at least 1)');
      return;
    }

    if (!confirm(`Update view limit for ${requesterId} to ${newLimit} views?`)) {
      return;
    }

    try {
      const response = await accessApi.manageAccess(imageId, requesterId, 'modify', newLimit);
      if (response.success) {
        alert(response.message || 'Access updated successfully');
        loadSharedImages();
      } else {
        alert(response.error || 'Failed to update access');
      }
    } catch (err: any) {
      alert(err.response?.data?.error || 'Failed to update access');
    }
  };

  const handleRevokeAccess = async (imageId: string, requesterId: string) => {
    if (!confirm(`Revoke access for ${requesterId}? This will delete their local copy.`)) {
      return;
    }

    try {
      const response = await accessApi.manageAccess(imageId, requesterId, 'revoke');
      if (response.success) {
        alert(response.message || 'Access revoked successfully');
        loadSharedImages();
      } else {
        alert(response.error || 'Failed to revoke access');
      }
    } catch (err: any) {
      alert(err.response?.data?.error || 'Failed to revoke access');
    }
  };

  useEffect(() => {
    loadSharedImages();
  }, []);

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-3xl font-bold text-gray-900">Manage Shared Images</h1>
          <p className="mt-1 text-sm text-gray-500">
            Control access to images you've shared
          </p>
        </div>
        <Button
          variant="outline"
          onClick={loadSharedImages}
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
          <CardTitle>Shared Images</CardTitle>
          <CardDescription>
            {images.length} image{images.length !== 1 ? 's' : ''} shared
          </CardDescription>
        </CardHeader>
        <CardContent>
          {isLoading ? (
            <div className="flex items-center justify-center py-12">
              <Loader2 className="h-8 w-8 animate-spin text-blue-600" />
            </div>
          ) : images.length === 0 ? (
            <div className="flex flex-col items-center justify-center py-12 text-center">
              <ImageOff className="h-12 w-12 text-gray-400 mb-4" />
              <p className="text-gray-600 font-medium">No shared images yet</p>
              <p className="text-sm text-gray-500 mt-1">
                Approve access requests to share images
              </p>
            </div>
          ) : (
            <div className="space-y-6">
              {images.map((image) => (
                <Card key={image.image_id} className="bg-gray-50">
                  <CardHeader>
                    <div className="flex items-center justify-between">
                      <div>
                        <CardTitle className="text-lg">{image.name}</CardTitle>
                        <CardDescription>
                          Shared with {image.total_shared_with} user{image.total_shared_with !== 1 ? 's' : ''}
                        </CardDescription>
                      </div>
                    </div>
                  </CardHeader>
                  <CardContent className="space-y-3">
                    {image.access_list.map((access) => {
                      const key = `${image.image_id}-${access.requester_id}`;
                      return (
                        <Card key={key} className="bg-white">
                          <CardContent className="p-4">
                            <div className="flex items-center gap-4">
                              <div className="flex-1">
                                <p className="font-medium text-gray-900">
                                  {access.requester_id}
                                </p>
                                <p className="text-sm text-gray-500">
                                  View Limit: {access.view_limit}
                                </p>
                              </div>
                              <div className="flex items-center gap-2">
                                <Label htmlFor={`limit-${key}`} className="sr-only">
                                  New Limit
                                </Label>
                                <Input
                                  id={`limit-${key}`}
                                  type="number"
                                  min="1"
                                  value={editingLimits[key] || access.view_limit}
                                  onChange={(e) =>
                                    setEditingLimits({
                                      ...editingLimits,
                                      [key]: parseInt(e.target.value) || 1,
                                    })
                                  }
                                  className="w-24"
                                />
                                <Button
                                  size="sm"
                                  variant="outline"
                                  onClick={() =>
                                    handleModifyAccess(image.image_id, access.requester_id)
                                  }
                                >
                                  <Edit2 className="h-4 w-4" />
                                </Button>
                                <Button
                                  size="sm"
                                  variant="destructive"
                                  onClick={() =>
                                    handleRevokeAccess(image.image_id, access.requester_id)
                                  }
                                >
                                  <Trash2 className="h-4 w-4" />
                                </Button>
                              </div>
                            </div>
                          </CardContent>
                        </Card>
                      );
                    })}
                  </CardContent>
                </Card>
              ))}
            </div>
          )}
        </CardContent>
      </Card>
    </div>
  );
}
