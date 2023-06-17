#pragma once

/**************************************************************************************
* OpenDraft:    Core System Resource Manager Class                                    *
*-------------------------------------------------------------------------------------*
* Filename:     OdSyResourceManager.h                                                 *
* Contributors: James Hodgkins                                                        *
* Date:         June 17, 2023                                                         *
* Copyright:    ©2023 OpenDraft. All Rights Reserved.                                 *
*-------------------------------------------------------------------------------------*
* Description:                                                                        *
*   Central manager for system resources (images, fonts etc)                          *
***************************************************************************************/



#include <unordered_map>
#include "System/Utils/OdSyImage.h"


class OdSyResourceManager
{
public:
	std::unordered_map<std::string, OdSyImage*> images;

	// Import an image into the resource manager
	// If an image with the same reference already exists, return a pointer to it
	// Otherwise, create a new image and store it in the resource manager, then return a pointer to it
	OdSyImage* importImageFromFile(NVGcontext* vg, int width, int height, const char* reference, const char* filePath)
	{
		// Check if an image with the given reference already exists
		std::unordered_map<std::string, OdSyImage*>::iterator imageIter = images.find(reference);
		if (imageIter != images.end())
		{
			// Image with the same reference already exists, return a pointer to it
			return imageIter->second;
		}

		// Image with the given reference does not exist, create a new image
		int imageHandle = nvgCreateImage(vg, filePath, 0);
		OdSyImage* newImage = new OdSyImage(width, height, 1.0, filePath, imageHandle);

		// Store the new image in the resource manager
		images[reference] = newImage;

		// Return a pointer to the new image
		return newImage;
	}
};