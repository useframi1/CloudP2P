"use client";

import {useState, useEffect} from "react";
import {useRouter} from "next/navigation";
import {useAuthStore} from "@/lib/store/auth";
import {Button} from "@/components/ui/button";
import {Input} from "@/components/ui/input";
import {Label} from "@/components/ui/label";
import {
    Card,
    CardContent,
    CardDescription,
    CardHeader,
    CardTitle,
} from "@/components/ui/card";
import {Tabs, TabsContent, TabsList, TabsTrigger} from "@/components/ui/tabs";
import {Alert, AlertDescription} from "@/components/ui/alert";
import {Cloud, Loader2} from "lucide-react";

export default function AuthPage() {
    const router = useRouter();
    const {isAuthenticated, isLoading, error, signIn, signUp, clearError} =
        useAuthStore();
    const [signInId, setSignInId] = useState("");
    const [signUpId, setSignUpId] = useState("");

    useEffect(() => {
        if (isAuthenticated) {
            router.push("/dashboard");
        }
    }, [isAuthenticated, router]);

    const handleSignIn = async (e: React.FormEvent) => {
        e.preventDefault();
        const success = await signIn(signInId);
        if (success) {
            router.push("/dashboard");
        }
    };

    const handleSignUp = async (e: React.FormEvent) => {
        e.preventDefault();
        const success = await signUp(signUpId);
        if (success) {
            router.push("/dashboard");
        }
    };

    return (
        <div className="flex min-h-screen items-center justify-center bg-linear-to-br from-blue-50 via-white to-gray-50 p-4">
            <div className="w-full max-w-md">
                <div className="mb-8 text-center">
                    <div className="mx-auto mb-4 flex h-16 w-16 items-center justify-center rounded-full bg-blue-600">
                        <Cloud className="h-8 w-8 text-white" />
                    </div>
                    <h1 className="text-3xl font-bold text-gray-900">
                        CloudP2P
                    </h1>
                    <p className="mt-2 text-sm text-gray-600">
                        Distributed Image Sharing Platform
                    </p>
                </div>

                <Card className="shadow-xl">
                    <CardHeader>
                        <CardTitle>Welcome</CardTitle>
                        <CardDescription>
                            Sign in to your account or create a new one
                        </CardDescription>
                    </CardHeader>
                    <CardContent>
                        {error && (
                            <Alert variant="destructive" className="mb-4">
                                <AlertDescription>{error}</AlertDescription>
                            </Alert>
                        )}

                        <Tabs defaultValue="signin" onValueChange={clearError}>
                            <TabsList className="grid w-full grid-cols-2">
                                <TabsTrigger value="signin">
                                    Sign In
                                </TabsTrigger>
                                <TabsTrigger value="signup">
                                    Sign Up
                                </TabsTrigger>
                            </TabsList>

                            <TabsContent value="signin">
                                <form
                                    onSubmit={handleSignIn}
                                    className="space-y-4"
                                >
                                    <div className="space-y-2">
                                        <Label htmlFor="signin-id">
                                            Client ID
                                        </Label>
                                        <Input
                                            id="signin-id"
                                            placeholder="Enter your client ID"
                                            value={signInId}
                                            onChange={(e) =>
                                                setSignInId(e.target.value)
                                            }
                                            required
                                            disabled={isLoading}
                                        />
                                    </div>
                                    <Button
                                        type="submit"
                                        className="w-full bg-blue-600 hover:bg-blue-700"
                                        disabled={isLoading}
                                    >
                                        {isLoading ? (
                                            <>
                                                <Loader2 className="mr-2 h-4 w-4 animate-spin" />
                                                Signing in...
                                            </>
                                        ) : (
                                            "Sign In"
                                        )}
                                    </Button>
                                </form>
                            </TabsContent>

                            <TabsContent value="signup">
                                <form
                                    onSubmit={handleSignUp}
                                    className="space-y-4"
                                >
                                    <div className="space-y-2">
                                        <Label htmlFor="signup-id">
                                            Client ID
                                        </Label>
                                        <Input
                                            id="signup-id"
                                            placeholder="Choose a client ID"
                                            value={signUpId}
                                            onChange={(e) =>
                                                setSignUpId(e.target.value)
                                            }
                                            required
                                            disabled={isLoading}
                                        />
                                        <p className="text-xs text-gray-500">
                                            Choose a unique identifier for your
                                            account
                                        </p>
                                    </div>
                                    <Button
                                        type="submit"
                                        className="w-full bg-blue-600 hover:bg-blue-700"
                                        disabled={isLoading}
                                    >
                                        {isLoading ? (
                                            <>
                                                <Loader2 className="mr-2 h-4 w-4 animate-spin" />
                                                Creating account...
                                            </>
                                        ) : (
                                            "Create Account"
                                        )}
                                    </Button>
                                </form>
                            </TabsContent>
                        </Tabs>
                    </CardContent>
                </Card>

                <p className="mt-4 text-center text-xs text-gray-500">
                    Fault-Tolerant P2P System with DoS
                </p>
            </div>
        </div>
    );
}
