import type { NextConfig } from "next";

const nextConfig: NextConfig = {
  async rewrites() {
    return [
      {
        source: '/api/:path*',
        destination: 'http://localhost:3001/api/:path*',
      },
      {
        source: '/client_images/:path*',
        destination: 'http://localhost:3001/client_images/:path*',
      },
    ];
  },
};

export default nextConfig;
