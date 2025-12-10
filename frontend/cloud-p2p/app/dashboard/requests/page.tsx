'use client';

import { useState, useEffect } from 'react';
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { accessApi } from '@/lib/api/client';
import { RefreshCw, Check, X, Loader2, Inbox } from 'lucide-react';
import { Alert, AlertDescription } from '@/components/ui/alert';

interface PendingRequest {
  request_id: string;
  requester_id: string;
  image_id: string;
  req_access_limit: number;
}

export default function RequestsPage() {
  const [requests, setRequests] = useState<PendingRequest[]>([]);
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [viewLimits, setViewLimits] = useState<Record<string, number>>({});
  const [processing, setProcessing] = useState<string | null>(null);

  const loadRequests = async () => {
    setIsLoading(true);
    setError(null);
    try {
      const response = await accessApi.getPendingRequests();
      if (response.success && response.requests) {
        setRequests(response.requests);
        // Initialize view limits with requested limits
        const limits: Record<string, number> = {};
        response.requests.forEach((req: PendingRequest) => {
          limits[req.request_id] = req.req_access_limit || 3;
        });
        setViewLimits(limits);
      } else {
        setError(response.error || 'Failed to load requests');
      }
    } catch (err: any) {
      setError(err.message || 'Failed to load requests');
    } finally {
      setIsLoading(false);
    }
  };

  const handleApprove = async (requestId: string) => {
    const viewLimit = viewLimits[requestId];
    if (!viewLimit || viewLimit < 1) {
      alert('Please enter a valid view limit');
      return;
    }

    setProcessing(requestId);
    try {
      const response = await accessApi.respondToRequest(requestId, true, viewLimit);
      if (response.success) {
        alert(response.message || 'Request approved successfully');
        loadRequests();
      } else {
        alert(response.error || 'Failed to approve request');
      }
    } catch (err: any) {
      alert(err.response?.data?.error || 'Failed to approve request');
    } finally {
      setProcessing(null);
    }
  };

  const handleDeny = async (requestId: string) => {
    if (!confirm('Are you sure you want to deny this request?')) {
      return;
    }

    setProcessing(requestId);
    try {
      const response = await accessApi.respondToRequest(requestId, false, null);
      if (response.success) {
        alert(response.message || 'Request denied');
        loadRequests();
      } else {
        alert(response.error || 'Failed to deny request');
      }
    } catch (err: any) {
      alert(err.response?.data?.error || 'Failed to deny request');
    } finally {
      setProcessing(null);
    }
  };

  useEffect(() => {
    loadRequests();
  }, []);

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-3xl font-bold text-gray-900">Pending Requests</h1>
          <p className="mt-1 text-sm text-gray-500">
            Approve or deny access requests from other users
          </p>
        </div>
        <Button
          variant="outline"
          onClick={loadRequests}
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
          <CardTitle>Access Requests</CardTitle>
          <CardDescription>
            {requests.length} pending request{requests.length !== 1 ? 's' : ''}
          </CardDescription>
        </CardHeader>
        <CardContent>
          {isLoading ? (
            <div className="flex items-center justify-center py-12">
              <Loader2 className="h-8 w-8 animate-spin text-blue-600" />
            </div>
          ) : requests.length === 0 ? (
            <div className="flex flex-col items-center justify-center py-12 text-center">
              <Inbox className="h-12 w-12 text-gray-400 mb-4" />
              <p className="text-gray-600 font-medium">No pending requests</p>
              <p className="text-sm text-gray-500 mt-1">
                You'll see access requests here when other users request your images
              </p>
            </div>
          ) : (
            <div className="space-y-4">
              {requests.map((request) => (
                <Card key={request.request_id} className="bg-gray-50">
                  <CardContent className="p-4">
                    <div className="flex items-center gap-4">
                      <div className="flex-1">
                        <p className="font-medium text-gray-900">
                          {request.requester_id}
                        </p>
                        <p className="text-sm text-gray-600 mt-1">
                          Image: <span className="font-mono">{request.image_id}</span>
                        </p>
                        <p className="text-xs text-gray-500 mt-1">
                          Requested limit: {request.req_access_limit} views
                        </p>
                      </div>
                      <div className="flex items-center gap-2">
                        <div className="flex flex-col gap-1">
                          <Label htmlFor={`limit-${request.request_id}`} className="text-xs">
                            View Limit
                          </Label>
                          <Input
                            id={`limit-${request.request_id}`}
                            type="number"
                            min="1"
                            value={viewLimits[request.request_id] || 3}
                            onChange={(e) =>
                              setViewLimits({
                                ...viewLimits,
                                [request.request_id]: parseInt(e.target.value) || 1,
                              })
                            }
                            className="w-20"
                            disabled={processing === request.request_id}
                          />
                        </div>
                        <div className="flex gap-2 mt-5">
                          <Button
                            size="sm"
                            className="bg-green-600 hover:bg-green-700"
                            onClick={() => handleApprove(request.request_id)}
                            disabled={processing !== null}
                          >
                            {processing === request.request_id ? (
                              <Loader2 className="h-4 w-4 animate-spin" />
                            ) : (
                              <Check className="h-4 w-4" />
                            )}
                          </Button>
                          <Button
                            size="sm"
                            variant="destructive"
                            onClick={() => handleDeny(request.request_id)}
                            disabled={processing !== null}
                          >
                            {processing === request.request_id ? (
                              <Loader2 className="h-4 w-4 animate-spin" />
                            ) : (
                              <X className="h-4 w-4" />
                            )}
                          </Button>
                        </div>
                      </div>
                    </div>
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
