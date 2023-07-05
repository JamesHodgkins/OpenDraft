#ifndef OD_DAT_DRAWINGDB_CPP
#define OD_DAT_DRAWINGDB_CPP

/**************************************************************************************
* OpenDraft:    Drawing Database Class                                                *
*-------------------------------------------------------------------------------------*
* Filename:     OdDrawingDb.cpp                                                       *
* Contributors: James Hodgkins                                                        *
* Date:         June 28, 2023                                                         *
* Copyright:    ©2023 OpenDraft. All Rights Reserved.                                 *
*-------------------------------------------------------------------------------------*
* Description:                                                                        *
*   Document class for OpenDraft framework.                                           *
***************************************************************************************/


#include "DataManager/OdDrawingDb.h"

namespace OD::Data
{

	// Add Entity
	void OdDrawingDb::AddCreatedEntity(OdDbObject* aObject)
	{
		// Iterate through objects and find the lowest unused handle
		int handle = 1;
		for (auto& object : objects)
		{
			// Check if object handle matches
			if (object->getHandle() == handle)
			{
				// Increment handle
				handle++;
			}
		}

		// Set object handle
		aObject->setHandle(handle);

		// Add object to vector
		objects.push_back(aObject);
	}

	// Remove Entity
	void OdDrawingDb::RemoveEntity(OdDbObject* aObject)
	{
		// Remove object from vector
		objects.erase(std::remove(objects.begin(), objects.end(), aObject), objects.end());
	}

	// Get Entity
	OdDbObject* OdDrawingDb::GetEntity(int aHandle)
	{
		// Loop through objects
		for (auto& object : objects)
		{
			// Check if object handle matches
			if (object->getHandle() == aHandle)
			{
				// Return object
				return object;
			}
		}

		// Return null
		return nullptr;

	}

}


#endif // OD_DAT_DRAWINGDB_CPP
