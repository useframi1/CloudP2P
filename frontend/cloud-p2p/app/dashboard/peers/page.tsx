'use client';

import { useState, useEffect } from 'react';
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card';
import { Button } from '@/components/ui/button';
import { Label } from '@/components/ui/label';
import { Badge } from '@/components/ui/badge';
import { peerApi, accessApi } from '@/lib/api/client';
import { RefreshCw, Users, Send, Loader2, ChevronRight } from 'lucide-react';
import { Alert, AlertDescription } from '@/components/ui/alert';
import { useAuthStore } from '@/lib/store/auth';
import { useAutoRefresh } from '@/lib/hooks/useAutoRefresh';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select';
import { Input } from '@/components/ui/input';

interface Peer {
  client_id: string;
  ip_address: string;
  images: Record<string, any>;
  online: boolean;
}

interface PeerImage {
  image_id: string;
  name: string;
}

export default function PeersPage() {
  const { clientId } = useAuthStore();
  const [peers, setPeers] = useState<Peer[]>([]);
  const [selectedPeer, setSelectedPeer] = useState<Peer | null>(null);
  const [peerImages, setPeerImages] = useState<PeerImage[]>([]);
  const [isLoading, setIsLoading] = useState(false);
  const [isLoadingImages, setIsLoadingImages] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [success, setSuccess] = useState<string | null>(null);
  const [selectedImageId, setSelectedImageId] = useState<string>('');
  const [viewLimit, setViewLimit] = useState<number>(3);
  const [requesting, setRequesting] = useState(false);

  const loadPeers = async (silent = false) => {
    if (!silent) {
      setIsLoading(true);
    }
    setError(null);
    try {
      const response = await peerApi.getAllPeers();
      if (response.success && response.peers) {
        // Filter out the current user from the peers list
        const filteredPeers = response.peers.filter(
          (peer: Peer) => peer.client_id !== clientId
        );
        setPeers(filteredPeers);
      } else {
        setError(response.error || 'Failed to load peers');
      }
    } catch (err: any) {
      setError(err.message || 'Failed to load peers');
    } finally {
      if (!silent) {
        setIsLoading(false);
      }
    }
  };

  const loadPeerImages = async (peerId: string) => {
    setIsLoadingImages(true);
    setError(null);
    try {
      const response = await peerApi.getPeerImages(peerId);
      if (response.success && response.images) {
        setPeerImages(response.images);
        setSelectedImageId(''); // Reset selection
      } else {
        setError(response.error || 'Failed to load peer images');
        setPeerImages([]);
      }
    } catch (err: any) {
      setError(err.message || 'Failed to load peer images');
      setPeerImages([]);
    } finally {
      setIsLoadingImages(false);
    }
  };

  const handlePeerSelect = (peer: Peer) => {
    setSelectedPeer(peer);
    setSuccess(null);
    setError(null);
    loadPeerImages(peer.client_id);
  };

  const handleRequestAccess = async () => {
    if (!selectedPeer || !selectedImageId) {
      setError('Please select an image');
      return;
    }

    setRequesting(true);
    setError(null);
    setSuccess(null);

    try {
      const response = await accessApi.requestAccess(
        selectedPeer.client_id,
        selectedImageId,
        viewLimit
      );
      if (response.success) {
        setSuccess(response.message || 'Access request sent successfully!');
        setSelectedImageId('');
      } else {
        setError(response.error || 'Failed to send request');
      }
    } catch (err: any) {
      setError(err.response?.data?.error || 'Failed to send request');
    } finally {
      setRequesting(false);
    }
  };

  useEffect(() => {
    loadPeers();
  }, []);

  // Auto-refresh every 5 seconds (silent mode)
  useAutoRefresh(() => loadPeers(true), 5000);

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-3xl font-bold text-gray-900">All Peers</h1>
          <p className="mt-1 text-sm text-gray-500">
            Select a peer to view their images and request access (works for online and offline users)
          </p>
        </div>
        <Button
          variant="outline"
          onClick={() => loadPeers()}
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

      {success && (
        <Alert className="border-green-200 bg-green-50 text-green-800">
          <AlertDescription>{success}</AlertDescription>
        </Alert>
      )}

      <div className="grid grid-cols-1 lg:grid-cols-3 gap-6">
        {/* Peers List */}
        <Card className="lg:col-span-1">
          <CardHeader>
            <CardTitle>All Users</CardTitle>
            <CardDescription>
              {peers.length} user{peers.length !== 1 ? 's' : ''} registered
            </CardDescription>
          </CardHeader>
          <CardContent>
            {isLoading ? (
              <div className="flex items-center justify-center py-12">
                <Loader2 className="h-8 w-8 animate-spin text-blue-600" />
              </div>
            ) : peers.length === 0 ? (
              <div className="flex flex-col items-center justify-center py-12 text-center">
                <Users className="h-12 w-12 text-gray-400 mb-4" />
                <p className="text-gray-600 font-medium">No users registered</p>
                <p className="text-sm text-gray-500 mt-1">
                  No other users have signed up yet
                </p>
              </div>
            ) : (
              <div className="space-y-2">
                {peers.map((peer) => (
                  <Card
                    key={peer.client_id}
                    className={`cursor-pointer transition-all ${
                      selectedPeer?.client_id === peer.client_id
                        ? 'bg-blue-50 border-blue-300 shadow-sm'
                        : 'bg-gray-50 hover:bg-gray-100'
                    }`}
                    onClick={() => handlePeerSelect(peer)}
                  >
                    <CardContent className="p-4">
                      <div className="flex items-center justify-between">
                        <div className="flex-1">
                          <p className="font-medium text-gray-900">
                            {peer.client_id}
                          </p>
                          <p className="text-xs text-gray-500 mt-1">
                            {Object.keys(peer.images || {}).length} images
                          </p>
                        </div>
                        <div className="flex items-center gap-2">
                          <Badge className={peer.online ? "bg-green-500" : "bg-gray-400"}>
                            {peer.online ? "Online" : "Offline"}
                          </Badge>
                          {selectedPeer?.client_id === peer.client_id && (
                            <ChevronRight className="h-4 w-4 text-blue-600" />
                          )}
                        </div>
                      </div>
                    </CardContent>
                  </Card>
                ))}
              </div>
            )}
          </CardContent>
        </Card>

        {/* Request Form */}
        <Card className="lg:col-span-2">
          <CardHeader>
            <CardTitle>Request Image Access</CardTitle>
            <CardDescription>
              {selectedPeer
                ? `Select an image from ${selectedPeer.client_id}`
                : 'Select a peer to view their images'}
            </CardDescription>
          </CardHeader>
          <CardContent>
            {!selectedPeer ? (
              <div className="flex flex-col items-center justify-center py-20 text-center">
                <Users className="h-16 w-16 text-gray-300 mb-4" />
                <p className="text-gray-500 font-medium">No peer selected</p>
                <p className="text-sm text-gray-400 mt-1">
                  Click on a peer from the list to see their images
                </p>
              </div>
            ) : isLoadingImages ? (
              <div className="flex items-center justify-center py-20">
                <Loader2 className="h-8 w-8 animate-spin text-blue-600" />
              </div>
            ) : peerImages.length === 0 ? (
              <div className="flex flex-col items-center justify-center py-20 text-center">
                <Users className="h-16 w-16 text-gray-300 mb-4" />
                <p className="text-gray-500 font-medium">No images available</p>
                <p className="text-sm text-gray-400 mt-1">
                  This peer hasn't registered any images yet
                </p>
              </div>
            ) : (
              <div className="space-y-6">
                <div className="space-y-2">
                  <Label htmlFor="image-select">Select Image</Label>
                  <Select value={selectedImageId} onValueChange={setSelectedImageId}>
                    <SelectTrigger id="image-select">
                      <SelectValue placeholder="Choose an image..." />
                    </SelectTrigger>
                    <SelectContent>
                      {peerImages.map((image) => (
                        <SelectItem key={image.image_id} value={image.image_id}>
                          {image.name}
                        </SelectItem>
                      ))}
                    </SelectContent>
                  </Select>
                  <p className="text-xs text-gray-500">
                    {peerImages.length} image{peerImages.length !== 1 ? 's' : ''} available
                  </p>
                </div>

                <div className="space-y-2">
                  <Label htmlFor="view-limit">Requested View Limit</Label>
                  <Input
                    id="view-limit"
                    type="number"
                    min="1"
                    value={viewLimit}
                    onChange={(e) => setViewLimit(parseInt(e.target.value) || 1)}
                  />
                  <p className="text-xs text-gray-500">
                    Number of times you want to view the image
                  </p>
                </div>

                <Button
                  onClick={handleRequestAccess}
                  className="w-full bg-blue-600 hover:bg-blue-700"
                  disabled={requesting || !selectedImageId}
                >
                  {requesting ? (
                    <>
                      <Loader2 className="mr-2 h-4 w-4 animate-spin" />
                      Sending Request...
                    </>
                  ) : (
                    <>
                      <Send className="mr-2 h-4 w-4" />
                      Send Request
                    </>
                  )}
                </Button>
              </div>
            )}
          </CardContent>
        </Card>
      </div>
    </div>
  );
}
