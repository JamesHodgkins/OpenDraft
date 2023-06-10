#include "System/OdSyApplication.h"


#include <Windows.h>
#include <iostream>
#include <vector>


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