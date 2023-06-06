#include "System/GrCore.h"

// Main code
int main(int, char**)
{
	GrApplication* app = new GrApplication();

	// Main loop
	while (app->isRunning())
	{
		app->update();
	}

	delete app;

	return 0;
}