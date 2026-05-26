import { usePlatform } from "@/hooks/use-platform";
import { useSettings } from "@/hooks/use-settings";
import { useSettingsContext } from "@/lib/settings-provider";
import { Button } from "@mizan/ui/components/ui/button";
import { Icons } from "@mizan/ui/components/ui/icons";
import { AnimatePresence, motion } from "motion/react";
import { useRef, useState } from "react";
import { Navigate } from "react-router-dom";
import { OnboardingAppearance, OnboardingAppearanceHandle } from "./onboarding-appearance";
import { OnboardingSignIn } from "./onboarding-signin";
import { OnboardingStep1 } from "./onboarding-step1";
import { OnboardingStep2, OnboardingStep2Handle } from "./onboarding-step2";

// AI-Native-3 — insert a new step 2 ("Sign in to Mizan Connect") so
// new users discover managed AI + cross-device sync during onboarding
// rather than finding it buried in settings. Skipping is a first-class
// path; the rest of the app is identical either way.
const DESKTOP_MAX_STEPS = 4;
const MOBILE_MAX_STEPS = 4;

const OnboardingPage = () => {
  const { data: settings, isLoading: isSettingsLoading } = useSettings();
  const { isMobile } = usePlatform();
  const { updateSettings } = useSettingsContext();
  const [currentStep, setCurrentStep] = useState(1);
  const [isStepValid, setIsStepValid] = useState(true);
  const settingsStepRef = useRef<OnboardingStep2Handle>(null);
  const appearanceStepRef = useRef<OnboardingAppearanceHandle>(null);
  const maxSteps = isMobile ? MOBILE_MAX_STEPS : DESKTOP_MAX_STEPS;
  const completionRoute = isMobile ? "/settings" : "/settings/accounts";
  const isFinalStep = currentStep === maxSteps;

  if (isSettingsLoading) return null;
  if (settings?.onboardingCompleted) {
    return <Navigate to={completionRoute} replace />;
  }

  const handleNext = () => {
    setCurrentStep((prev) => Math.min(prev + 1, maxSteps));
  };

  const handleBack = () => {
    setCurrentStep((prev) => Math.max(prev - 1, 1));
  };

  const handleContinue = () => {
    // Step ordering after AI-Native-3:
    //   1) Welcome
    //   2) Sign-in (optional — Continue skips it; the in-step button
    //      signs in then advances)
    //   3) Currency / timezone (ref-driven submit)
    //   4) Appearance (ref-driven submit)
    if (currentStep === 3 && settingsStepRef.current) {
      settingsStepRef.current.submitForm();
    } else if (currentStep === 4 && appearanceStepRef.current) {
      appearanceStepRef.current.submitForm();
    } else {
      handleNext();
    }
  };

  return (
    <div className="bg-background flex h-screen flex-col pt-[env(safe-area-inset-top)]">
      {/* Fixed Header with Logo and Steppers */}
      <header className="flex-none px-4 pt-8 sm:px-6 sm:pt-12">
        <div className="flex flex-col items-center">
          {/* Logo */}
          <img alt="Mizan" className="mb-3 h-16 w-16 sm:h-20 sm:w-20" src="/logo-vantage.png" />

          {/* Progress indicators */}
          <div className="flex gap-2">
            {Array.from({ length: maxSteps }).map((_, index) => (
              <div
                key={index}
                className={`h-1.5 rounded-full transition-all duration-300 ${
                  index === currentStep - 1
                    ? "bg-primary w-8"
                    : index < currentStep - 1
                      ? "bg-primary/50 w-1.5"
                      : "bg-muted w-1.5"
                }`}
              />
            ))}
          </div>
        </div>
      </header>

      {/* Main content - centered vertically in remaining space */}
      <main className="flex flex-1 flex-col items-center justify-center overflow-y-auto px-4 sm:px-6">
        <AnimatePresence mode="wait" initial={false}>
          <motion.div
            key={currentStep}
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
            transition={{ duration: 0.15 }}
            className="flex w-full max-w-4xl justify-center"
          >
            {currentStep === 1 && <OnboardingStep1 />}
            {currentStep === 2 && (
              <OnboardingSignIn onSkip={handleNext} onSignedIn={handleNext} />
            )}
            {currentStep === 3 && (
              <OnboardingStep2
                ref={settingsStepRef}
                onNext={handleNext}
                onValidityChange={setIsStepValid}
              />
            )}
            {currentStep === 4 && (
              <OnboardingAppearance
                ref={appearanceStepRef}
                onNext={handleNext}
                onValidityChange={setIsStepValid}
              />
            )}
          </motion.div>
        </AnimatePresence>
      </main>

      {/* Fixed Footer */}
      <footer className="flex-none pb-[env(safe-area-inset-bottom)]">
        <div className="sm:pb-18 mx-auto max-w-4xl px-4 pb-8 pt-6 sm:px-6">
          {isFinalStep ? (
            <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
              <div className="order-2 sm:order-1">
                <Button variant="ghost" onClick={handleBack} size="sm">
                  <Icons.ArrowLeft className="mr-1.5 h-4 w-4" />
                  Back
                </Button>
              </div>
              <div className="order-1 flex flex-col gap-2 sm:order-2 sm:flex-row sm:gap-3">
                <Button
                  data-testid="onboarding-finish-button"
                  className="from-primary to-primary/90 bg-linear-to-r order-1 sm:order-2"
                  onClick={() => updateSettings({ onboardingCompleted: true })}
                >
                  Get Started
                  <Icons.ArrowRight className="ml-1.5 h-4 w-4" />
                </Button>
              </div>
            </div>
          ) : (
            <div className="flex items-center justify-between">
              <div>
                {currentStep > 1 && (
                  <Button variant="ghost" onClick={handleBack} size="sm">
                    <Icons.ArrowLeft className="mr-1.5 h-4 w-4" />
                    Back
                  </Button>
                )}
              </div>
              <Button
                onClick={handleContinue}
                disabled={!isStepValid}
                className="from-primary to-primary/90 bg-linear-to-r"
              >
                Continue
                <Icons.ArrowRight className="ml-1.5 h-4 w-4" />
              </Button>
            </div>
          )}
        </div>
      </footer>
    </div>
  );
};

export default OnboardingPage;
