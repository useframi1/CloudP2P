'use client';

import Link from 'next/link';
import { usePathname } from 'next/navigation';
import { cn } from '@/lib/utils';
import { Button } from '@/components/ui/button';
import { ScrollArea } from '@/components/ui/scroll-area';
import { Separator } from '@/components/ui/separator';
import { Avatar, AvatarFallback } from '@/components/ui/avatar';
import { useAuthStore } from '@/lib/store/auth';
import {
  Home,
  ImageIcon,
  Share2,
  Users,
  Bell,
  LogOut,
  Cloud,
} from 'lucide-react';

const navigation = [
  { name: 'My Images', href: '/dashboard', icon: Home },
  { name: 'Shared With Me', href: '/dashboard/shared', icon: Share2 },
  { name: 'Manage Access', href: '/dashboard/manage', icon: ImageIcon },
  { name: 'Pending Requests', href: '/dashboard/requests', icon: Bell },
  { name: 'Online Peers', href: '/dashboard/peers', icon: Users },
];

export function Sidebar() {
  const pathname = usePathname();
  const { clientId, signOut } = useAuthStore();

  const handleSignOut = async () => {
    if (confirm('Are you sure you want to sign out?')) {
      await signOut();
      window.location.href = '/auth';
    }
  };

  return (
    <div className="flex h-screen w-64 flex-col border-r bg-white">
      {/* Logo */}
      <div className="flex h-16 items-center gap-2 border-b px-6">
        <Cloud className="h-6 w-6 text-blue-600" />
        <span className="text-lg font-semibold text-gray-900">CloudP2P</span>
      </div>

      {/* User Info */}
      <div className="flex items-center gap-3 border-b p-4">
        <Avatar>
          <AvatarFallback className="bg-blue-600 text-white">
            {clientId?.substring(0, 2).toUpperCase()}
          </AvatarFallback>
        </Avatar>
        <div className="flex-1 min-w-0">
          <p className="text-sm font-medium text-gray-900 truncate">{clientId}</p>
          <p className="text-xs text-gray-500">Active</p>
        </div>
      </div>

      {/* Navigation */}
      <ScrollArea className="flex-1 px-3 py-4">
        <nav className="space-y-1">
          {navigation.map((item) => {
            const isActive = pathname === item.href;
            return (
              <Link key={item.name} href={item.href}>
                <Button
                  variant={isActive ? 'default' : 'ghost'}
                  className={cn(
                    'w-full justify-start gap-3',
                    isActive
                      ? 'bg-blue-600 text-white hover:bg-blue-700'
                      : 'text-gray-700 hover:bg-gray-100'
                  )}
                >
                  <item.icon className="h-5 w-5" />
                  {item.name}
                </Button>
              </Link>
            );
          })}
        </nav>
      </ScrollArea>

      {/* Sign Out */}
      <div className="border-t p-4">
        <Button
          variant="ghost"
          className="w-full justify-start gap-3 text-red-600 hover:bg-red-50 hover:text-red-700"
          onClick={handleSignOut}
        >
          <LogOut className="h-5 w-5" />
          Sign Out
        </Button>
      </div>
    </div>
  );
}
