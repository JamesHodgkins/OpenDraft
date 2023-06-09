#include "System/OdSyApplication.h"

// Main code
int main(int, char**)
{
	OdGrApplication* app = new OdGrApplication();

	// Main loop
	while (app->isRunning())
	{
		app->update();
	}

	delete app;

	return 0;
}