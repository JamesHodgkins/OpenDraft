#include "System/OdApplication.h"


#include <Windows.h>
#include <iostream>
#include <vector>


// Main code
int main(int, char**)
{
	OdApplication* app = OdApplication::getInstance();

	// Main loop
	while (app->isRunning())
	{
		app->update();
	}

	// Release app
	delete app;

	return 0;
}