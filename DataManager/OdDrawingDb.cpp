#ifndef OD_DAT_DRAWINGDB_CPP
#define OD_DAT_DRAWINGDB_CPP

/**************************************************************************************
* OpenDraft:    Drawing Database Class                                                *
*-------------------------------------------------------------------------------------*
* Filename:     OdDrawingDb.cpp                                                       *
* Contributors: James Hodgkins                                                        *
* Date:         July 08, 2023                                                         *
* Copyright:    ©2023 OpenDraft. GPLv3                                                *
*-------------------------------------------------------------------------------------*
* Description:                                                                        *
*   Document class for OpenDraft framework.                                           *
***************************************************************************************/


#include "DataManager/OdDrawingDb.h"
#include "System/Entities/OdEntity.h"

namespace OD::Data
{
	// Constructor and Destructor
	OdDrawingDb::OdDrawingDb()
	{

	}

	OdDrawingDb::~OdDrawingDb()
	{
		// Delete all objects
		for (auto& object : objects)
			delete object;
	}

	// Private Method: Request New Handle Id
	int OdDrawingDb::requestNewHandleId()
	{
		// Iterate through objects and find the lowest unused handle
		int handle = 1;

		for (auto& object : objects)
		{
			// Check if object handle matches
			if (object->getHandle() == handle)
				handle++;
		}

		return handle;
	}

	// Add DbObject Record
	void OdDrawingDb::addRecord(OdDbObject* aObject)
	{
		if (aObject == nullptr)
			return;

		// Add object to vector
		objects.push_back(aObject);
	}

	// Add Entity
	void OdDrawingDb::addCreatedEntity(OD::Geometry::OdEntity* aObject)
	{
		// Check if object is null
		if (aObject == nullptr)
			return;

		// Cast to OdDbObject
		OdDbObject* dbObject = dynamic_cast<OdDbObject*>(aObject);
		
		// Check if object can be cast to OdDbObject
		if (dbObject == nullptr)
			return;

		// Set object layer to default if null
		if (aObject->getLayer() == nullptr)
			aObject->setLayer("Undefined");

		// Request and set new handle
		int newHandle = requestNewHandleId();
		aObject->setHandle(newHandle);

		// Add object to vector
		objects.push_back(aObject);

	}

	// Remove Entity
	void OdDrawingDb::removeRecord(OdDbObject* aObject)
	{
		// Remove object from vector
		objects.erase(std::remove(objects.begin(), objects.end(), aObject), objects.end());
	}

	// Get Entity by Handle
	OdDbObject* OdDrawingDb::getRecordByHandle(int aHandle)
	{
		// Loop through objects
		for (auto& object : objects)
		{
			// Check if object handle matches
			if (object->getHandle() == aHandle)
				return object;
			
		}

		// Return null
		return nullptr;

	}

	// Get Entity by Index
	OdDbObject* OdDrawingDb::getRecordByIndex(int aIndex)
	{
		// Check if index is valid
		if (aIndex < 0 || aIndex >= objects.size())
			return nullptr;

		// Return object
		return objects[aIndex];
	}

	// Get Entity Count
	int OdDrawingDb::getRecordCount()
	{
		// Return object count
		return objects.size();
	}

}


#endif // !OD_DAT_DRAWINGDB_CPP
