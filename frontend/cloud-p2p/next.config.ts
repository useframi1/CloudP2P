import type { NextConfig } from "next";

const nextConfig: NextConfig = {
  async rewrites() {
    return [
      {
        source: '/api/:path*',
        destination: 'http://localhost:3001/api/:path*',
      },
      {
        source: '/test_images/:path*',
        destination: 'http://localhost:3001/test_images/:path*',
      },
      {
        source: '/encrypted_images/:path*',
        destination: 'http://localhost:3001/encrypted_images/:path*',
      },
    ];
  },
};

export default nextConfig;
