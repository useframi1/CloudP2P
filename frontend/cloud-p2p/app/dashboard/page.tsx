'use client';

import { useState, useEffect } from 'react';
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card';
import { Button } from '@/components/ui/button';
import { imageApi } from '@/lib/api/client';
import { RefreshCw, ImagePlus, Loader2, Upload } from 'lucide-react';
import { Alert, AlertDescription } from '@/components/ui/alert';
import { useAuthStore } from '@/lib/store/auth';
import { useAutoRefresh } from '@/lib/hooks/useAutoRefresh';
import { useRef } from 'react';

interface Image {
  image_id: string;
  name: string;
  encrypted_path: string;
}

export default function DashboardPage() {
  const { clientId } = useAuthStore();
  const fileInputRef = useRef<HTMLInputElement>(null);
  const [images, setImages] = useState<Image[]>([]);
  const [isLoading, setIsLoading] = useState(false);
  const [isRegistering, setIsRegistering] = useState(false);
  const [isUploading, setIsUploading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [success, setSuccess] = useState<string | null>(null);

  const loadImages = async (silent = false) => {
    if (!silent) {
      setIsLoading(true);
    }
    setError(null);
    try {
      const response = await imageApi.getMyImages();
      if (response.success && response.images) {
        setImages(response.images);
      } else {
        setError(response.error || 'Failed to load images');
      }
    } catch (err: any) {
      setError(err.message || 'Failed to load images');
    } finally {
      if (!silent) {
        setIsLoading(false);
      }
    }
  };

  const handleRegisterImages = async () => {
    setIsRegistering(true);
    setError(null);
    setSuccess(null);
    try {
      const response = await imageApi.registerImages();
      if (response.success) {
        setSuccess(response.message || 'Images registered successfully!');
        loadImages();
      } else {
        setError(response.error || 'Failed to register images');
      }
    } catch (err: any) {
      setError(err.message || 'Failed to register images');
    } finally {
      setIsRegistering(false);
    }
  };

  const handleUploadImage = async (event: React.ChangeEvent<HTMLInputElement>) => {
    const file = event.target.files?.[0];
    if (!file) return;

    // Validate file type
    if (!file.type.startsWith('image/')) {
      setError('Please select an image file');
      return;
    }

    setIsUploading(true);
    setError(null);
    setSuccess(null);

    try {
      const response = await imageApi.uploadImage(file);
      if (response.success) {
        setSuccess(response.message || 'Image uploaded and registered successfully!');
        loadImages();
        // Reset file input
        event.target.value = '';
      } else {
        setError(response.error || 'Failed to upload image');
      }
    } catch (err: any) {
      setError(err.response?.data?.error || err.message || 'Failed to upload image');
    } finally {
      setIsUploading(false);
    }
  };

  useEffect(() => {
    loadImages();
  }, []);

  // Auto-refresh every 5 seconds (silent mode)
  useAutoRefresh(() => loadImages(true), 5000);

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-3xl font-bold text-gray-900">My Images</h1>
          <p className="mt-1 text-sm text-gray-500">
            Manage and register your encrypted images
          </p>
        </div>
        <div className="flex gap-2">
          <Button
            variant="outline"
            onClick={() => loadImages()}
            disabled={isLoading}
          >
            <RefreshCw className={`h-4 w-4 ${isLoading ? 'animate-spin' : ''}`} />
          </Button>
          <Button
            onClick={handleRegisterImages}
            disabled={isRegistering}
            variant="outline"
          >
            {isRegistering ? (
              <>
                <Loader2 className="mr-2 h-4 w-4 animate-spin" />
                Registering...
              </>
            ) : (
              <>
                <ImagePlus className="mr-2 h-4 w-4" />
                Register All
              </>
            )}
          </Button>
          <input
            ref={fileInputRef}
            type="file"
            accept="image/*"
            onChange={handleUploadImage}
            className="hidden"
          />
          <Button
            onClick={() => fileInputRef.current?.click()}
            disabled={isUploading}
            className="bg-blue-600 hover:bg-blue-700"
          >
            {isUploading ? (
              <>
                <Loader2 className="mr-2 h-4 w-4 animate-spin" />
                Uploading...
              </>
            ) : (
              <>
                <Upload className="mr-2 h-4 w-4" />
                Upload Image
              </>
            )}
          </Button>
        </div>
      </div>

      {error && (
        <Alert variant="destructive">
          <AlertDescription>{error}</AlertDescription>
        </Alert>
      )}

      {success && (
        <Alert className="border-green-200 bg-green-50 text-green-800">
          <AlertDescription>{success}</AlertDescription>
        </Alert>
      )}

      <Card>
        <CardHeader>
          <CardTitle>Registered Images</CardTitle>
          <CardDescription>
            {images.length} image{images.length !== 1 ? 's' : ''} registered
          </CardDescription>
        </CardHeader>
        <CardContent>
          {isLoading ? (
            <div className="flex items-center justify-center py-12">
              <Loader2 className="h-8 w-8 animate-spin text-blue-600" />
            </div>
          ) : images.length === 0 ? (
            <div className="flex flex-col items-center justify-center py-12 text-center">
              <ImagePlus className="h-12 w-12 text-gray-400 mb-4" />
              <p className="text-gray-600 font-medium">No images registered yet</p>
              <p className="text-sm text-gray-500 mt-1">
                Click "Register Images" to encrypt and register your images
              </p>
            </div>
          ) : (
            <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
              {images.map((image) => (
                <Card key={image.image_id} className="overflow-hidden hover:shadow-md transition-shadow cursor-pointer group">
                  <div className="aspect-square bg-gray-100 relative overflow-hidden">
                    <img
                      src={`/client_images/${clientId}/original_images/${image.name}`}
                      alt={image.name}
                      className="w-full h-full object-cover transition-transform duration-300 group-hover:scale-105"
                      onError={(e) => {
                        const target = e.currentTarget;
                        target.style.display = 'none';
                        const parent = target.parentElement;
                        if (parent) {
                          parent.innerHTML = '<div class="w-full h-full flex items-center justify-center"><svg class="h-16 w-16 text-gray-300" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 16l4.586-4.586a2 2 0 012.828 0L16 16m-2-2l1.586-1.586a2 2 0 012.828 0L20 14m-6-6h.01M6 20h12a2 2 0 002-2V6a2 2 0 00-2-2H6a2 2 0 00-2 2v12a2 2 0 002 2z"></path></svg></div>';
                        }
                      }}
                    />
                  </div>
                  <CardContent className="p-4">
                    <p className="font-medium text-gray-900 truncate">{image.name}</p>
                    <p className="text-xs text-gray-500 mt-1 truncate">
                      ID: {image.image_id}
                    </p>
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
